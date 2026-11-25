//! Coins Manager do Lab: distribuições de fichas, na hora ou agendadas, para todos os jogadores ou
//! para um só, sempre com um motivo.
//!
//! - `coinGrants/{id}`: a distribuição (para quem, quantas, motivo, quando e situação).
//! - Crédito idempotente: cada jogador recebe o lançamento `wallets/{uid}/entries/grant-{id}`, criado
//!   na mesma transação que soma o saldo. Se o lançamento já existe, aquele jogador já recebeu: uma
//!   distribuição interrompida pode rodar de novo sem creditar duas vezes.
//! - Agendadas são creditadas pelo próprio backend quando chega a hora: [`acordar`] roda em qualquer
//!   requisição (no máximo a cada [`INTERVALO_DE_CONFERENCIA`]) e o Coins Manager confere ao abrir.
//!   O Cloud Run desliga sem tráfego; o crédito sai no primeiro acesso depois do horário (o Client
//!   aberto consulta o backend a cada poucos minutos).

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::AppError;
use crate::firestore::valor::{data, inteiro, ler_data, ler_inteiro, ler_texto, texto};
use crate::firestore::{Campos, Firestore};
use crate::services::gameplays::{id_valido, FICHAS_INICIAIS};

const COLECAO: &str = "coinGrants";

/// Motivos aceitos (o Lab mostra o nome de cada um). `other` exige observação.
pub const MOTIVOS: [&str; 10] = [
    "event_prize",
    "season_reward",
    "compensation",
    "support_refund",
    "promotion",
    "welcome_bonus",
    "bug_report",
    "community",
    "content_creator",
    "other",
];

/// Teto por jogador numa distribuição: um erro de digitação não vira milhões de fichas.
pub const MAXIMO_POR_JOGADOR: i64 = 10_000;
/// Até quando dá para agendar.
const MAXIMO_DE_ANTECEDENCIA_DIAS: i64 = 366;
/// Distribuição em `running` há mais que isso (instância caiu no meio): pode ser retomada.
const TRAVADA_APOS_MINUTOS: i64 = 10;
/// Intervalo mínimo entre conferências das agendadas pela mesma instância.
const INTERVALO_DE_CONFERENCIA: i64 = 20;
/// Créditos em paralelo (cada um é uma transação numa carteira diferente).
const CREDITOS_EM_PARALELO: usize = 8;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Distribuicao {
    pub id: String,
    /// `all` (todos os jogadores) ou `user` (um só).
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    /// Nome do jogador no momento do agendamento (para a lista do Lab).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_label: Option<String>,
    pub amount: i64,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub scheduled_at: DateTime<Utc>,
    /// `scheduled`, `running`, `done`, `cancelled` ou `failed`.
    pub status: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_at: Option<DateTime<Utc>>,
    /// Jogadores creditados até agora.
    pub recipients: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DistribuicaoEntrada {
    pub target: String,
    #[serde(default)]
    pub uid: Option<String>,
    pub amount: i64,
    pub reason: String,
    #[serde(default)]
    pub note: Option<String>,
    /// Ausente (ou já no passado recente) = agora.
    #[serde(default)]
    pub scheduled_at: Option<DateTime<Utc>>,
}

fn caminho(id: &str) -> String {
    format!("{COLECAO}/{id}")
}

fn distribuicao_de(id: String, c: &Campos) -> Distribuicao {
    Distribuicao {
        target: ler_texto(c, "target").unwrap_or_else(|| "user".into()),
        uid: ler_texto(c, "uid"),
        user_label: ler_texto(c, "userLabel"),
        amount: ler_inteiro(c, "amount").unwrap_or(0),
        reason: ler_texto(c, "reason").unwrap_or_else(|| "other".into()),
        note: ler_texto(c, "note"),
        scheduled_at: ler_data(c, "scheduledAt").unwrap_or_default(),
        status: ler_texto(c, "status").unwrap_or_else(|| "scheduled".into()),
        created_by: ler_texto(c, "createdBy").unwrap_or_default(),
        created_at: ler_data(c, "createdAt").unwrap_or_default(),
        executed_at: ler_data(c, "executedAt"),
        recipients: ler_inteiro(c, "recipients").unwrap_or(0),
        error: ler_texto(c, "error"),
        id,
    }
}

/// Confere a entrada e devolve (observação limpa, horário efetivo).
fn validar(entrada: &DistribuicaoEntrada, agora: DateTime<Utc>) -> Result<(Option<String>, DateTime<Utc>), AppError> {
    match entrada.target.as_str() {
        "all" if entrada.uid.is_some() => return Err(AppError::bad_request("para todos os jogadores, não informe o jogador")),
        "all" => {}
        "user" => match entrada.uid.as_deref() {
            Some(uid) if id_valido(uid) => {}
            _ => return Err(AppError::bad_request("informe o jogador que vai receber as fichas")),
        },
        _ => return Err(AppError::bad_request("destino inválido: use all ou user")),
    }
    if !(1..=MAXIMO_POR_JOGADOR).contains(&entrada.amount) {
        return Err(AppError::bad_request(format!("quantidade de fichas inválida: de 1 a {MAXIMO_POR_JOGADOR}")));
    }
    if !MOTIVOS.contains(&entrada.reason.as_str()) {
        return Err(AppError::bad_request("motivo inválido"));
    }
    let nota = entrada.note.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(str::to_string);
    if nota.as_ref().is_some_and(|n| n.chars().count() > 200) {
        return Err(AppError::bad_request("observação longa demais (até 200 caracteres)"));
    }
    if entrada.reason == "other" && nota.is_none() {
        return Err(AppError::bad_request("no motivo \"Outro\", escreva a observação"));
    }
    let quando = match entrada.scheduled_at {
        None => agora,
        // Um minuto de folga para o relógio do PC do admin e o tempo de clicar.
        Some(quando) if quando < agora - Duration::minutes(5) => {
            return Err(AppError::bad_request("o horário do agendamento já passou"));
        }
        Some(quando) if quando > agora + Duration::days(MAXIMO_DE_ANTECEDENCIA_DIAS) => {
            return Err(AppError::bad_request("agende para no máximo um ano à frente"));
        }
        Some(quando) => quando.max(agora),
    };
    Ok((nota, quando))
}

/// Cria a distribuição. Para agora, credita na hora e devolve já concluída.
pub async fn criar(fs: &Firestore, entrada: DistribuicaoEntrada, admin: &str) -> Result<Distribuicao, AppError> {
    let agora = Utc::now();
    let (nota, quando) = validar(&entrada, agora)?;
    let mut rotulo = None;
    if let Some(uid) = &entrada.uid {
        let lidos = fs.ler_varios(&[format!("users/{uid}"), format!("players/{uid}"), format!("publicProfiles/{uid}")]).await?;
        if lidos.is_empty() {
            return Err(AppError::not_found("jogador não encontrado"));
        }
        let nome = lidos.get(&format!("users/{uid}")).and_then(|c| ler_texto(c, "displayName"));
        let nick = lidos.get(&format!("publicProfiles/{uid}")).and_then(|c| ler_texto(c, "handle")).map(|h| format!("@{h}"));
        let email = lidos.get(&format!("players/{uid}")).and_then(|c| ler_texto(c, "email"));
        rotulo = Some([nome, nick].into_iter().flatten().collect::<Vec<_>>().join(" · ")).filter(|r| !r.is_empty()).or(email);
    }

    let id = Uuid::new_v4().to_string();
    let mut campos = Campos::new();
    campos.insert("target".into(), texto(&entrada.target));
    if let Some(uid) = &entrada.uid {
        campos.insert("uid".into(), texto(uid));
    }
    if let Some(rotulo) = &rotulo {
        campos.insert("userLabel".into(), texto(rotulo));
    }
    campos.insert("amount".into(), inteiro(entrada.amount));
    campos.insert("reason".into(), texto(&entrada.reason));
    if let Some(nota) = &nota {
        campos.insert("note".into(), texto(nota));
    }
    campos.insert("scheduledAt".into(), data(quando));
    campos.insert("status".into(), texto("scheduled"));
    campos.insert("createdBy".into(), texto(admin));
    campos.insert("createdAt".into(), data(agora));
    campos.insert("recipients".into(), inteiro(0));
    fs.gravar_novo(&caminho(&id), campos).await?;

    if quando <= Utc::now() {
        executar(fs, &id).await?;
    }
    obter(fs, &id).await
}

pub async fn obter(fs: &Firestore, id: &str) -> Result<Distribuicao, AppError> {
    if !id_valido(id) {
        return Err(AppError::bad_request("distribuição inválida"));
    }
    let campos = fs.ler_documento(&caminho(id)).await?.ok_or_else(|| AppError::not_found("distribuição não encontrada"))?;
    Ok(distribuicao_de(id.to_string(), &campos))
}

/// Todas as distribuições, da mais recente para a mais antiga (pelo horário marcado).
pub async fn listar(fs: &Firestore) -> Result<Vec<Distribuicao>, AppError> {
    let mut lista: Vec<Distribuicao> = fs.listar(COLECAO).await?.into_iter().map(|(id, c)| distribuicao_de(id, &c)).collect();
    lista.sort_by(|a, b| b.scheduled_at.cmp(&a.scheduled_at).then(b.created_at.cmp(&a.created_at)));
    Ok(lista)
}

/// Cancela uma distribuição ainda não creditada.
pub async fn cancelar(fs: &Firestore, id: &str) -> Result<Distribuicao, AppError> {
    let atual = obter(fs, id).await?;
    if atual.status != "scheduled" {
        return Err(AppError::conflict("só dá para cancelar uma distribuição ainda agendada"));
    }
    let mut campos = Campos::new();
    campos.insert("status".into(), texto("cancelled"));
    campos.insert("updatedAt".into(), data(Utc::now()));
    fs.gravar(&caminho(id), campos, &["status", "updatedAt"]).await?;
    obter(fs, id).await
}

/// Roda de novo uma distribuição que falhou: quem já recebeu não recebe outra vez.
pub async fn repetir(fs: &Firestore, id: &str) -> Result<Distribuicao, AppError> {
    let atual = obter(fs, id).await?;
    if atual.status != "failed" {
        return Err(AppError::conflict("só dá para repetir uma distribuição que falhou"));
    }
    let mut campos = Campos::new();
    campos.insert("status".into(), texto("scheduled"));
    campos.insert("scheduledAt".into(), data(Utc::now().min(atual.scheduled_at.max(atual.created_at))));
    fs.gravar(&caminho(id), campos, &["status", "scheduledAt"]).await?;
    executar(fs, id).await?;
    obter(fs, id).await
}

/// Ocupa a distribuição para creditar (`scheduled` vencida ou `running` travada viram `running`).
/// `false` se outra instância já está creditando, se ainda não venceu ou se acabou.
async fn ocupar(fs: &Firestore, id: &str, agora: DateTime<Utc>) -> Result<Option<Campos>, AppError> {
    let tx = fs.iniciar_transacao().await?;
    let resultado: Result<Option<Campos>, AppError> = async {
        let Some(campos) = fs.ler(&caminho(id), &tx).await? else { return Ok(None) };
        let pronta = match ler_texto(&campos, "status").as_deref() {
            Some("scheduled") => ler_data(&campos, "scheduledAt").is_some_and(|q| q <= agora),
            Some("running") => ler_data(&campos, "claimedAt").is_none_or(|q| q < agora - Duration::minutes(TRAVADA_APOS_MINUTOS)),
            _ => false,
        };
        if !pronta {
            return Ok(None);
        }
        let escrita = json!({
            "update": { "name": fs.nome(&caminho(id)), "fields": { "status": texto("running"), "claimedAt": data(agora) } },
            "updateMask": { "fieldPaths": ["status", "claimedAt"] },
        });
        fs.confirmar(&tx, vec![escrita]).await?;
        Ok(Some(campos))
    }
    .await;
    if !matches!(resultado, Ok(Some(_))) {
        fs.desfazer(&tx).await;
    }
    resultado
}

/// Jogadores que recebem uma distribuição para todos: quem tem conta (`users`), menos os banidos.
async fn todos_os_jogadores(fs: &Firestore) -> Result<Vec<String>, AppError> {
    let ids = fs.consultar(json!({ "from": [{ "collectionId": "users" }], "select": { "fields": [{ "fieldPath": "__name__" }] } })).await?;
    let banidos: std::collections::HashSet<String> = fs
        .consultar(json!({
            "from": [{ "collectionId": "moderation" }],
            "where": { "fieldFilter": { "field": { "fieldPath": "banned" }, "op": "EQUAL", "value": { "booleanValue": true } } },
        }))
        .await?
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    Ok(ids.into_iter().map(|(id, _)| id).filter(|id| id_valido(id) && !banidos.contains(id)).collect())
}

/// Credita as fichas da distribuição na carteira do jogador. `Ok(true)` = creditou agora;
/// `Ok(false)` = já tinha recebido esta distribuição. Carteira nova nasce com `FICHAS_INICIAIS`.
pub async fn creditar(fs: &Firestore, uid: &str, id: &str, quantidade: i64, motivo: &str, agora: DateTime<Utc>) -> Result<bool, AppError> {
    let tx = fs.iniciar_transacao().await?;
    let resultado: Result<bool, AppError> = async {
        let carteira_caminho = format!("wallets/{uid}");
        let lancamento = format!("{carteira_caminho}/entries/grant-{id}");
        if fs.ler(&lancamento, &tx).await?.is_some() {
            return Ok(false);
        }
        let carteira = fs.ler(&carteira_caminho, &tx).await?;
        let nova = carteira.is_none();
        let saldo = carteira.as_ref().and_then(|c| ler_inteiro(c, "balance")).unwrap_or(FICHAS_INICIAIS);
        let versao = carteira.as_ref().and_then(|c| ler_inteiro(c, "version")).unwrap_or(0);
        let saldo_final = saldo + quantidade;

        let mut escritas: Vec<Value> = Vec::new();
        let mut campos_carteira = serde_json::Map::new();
        campos_carteira.insert("balance".into(), inteiro(saldo_final));
        campos_carteira.insert("version".into(), inteiro(versao + 1));
        campos_carteira.insert("updatedAt".into(), data(agora));
        let mut mascara = vec!["balance", "version", "updatedAt"];
        if nova {
            campos_carteira.insert("createdAt".into(), data(agora));
            mascara.push("createdAt");
            escritas.push(json!({
                "update": { "name": fs.nome(&format!("{carteira_caminho}/entries/{}", Uuid::new_v4())), "fields": {
                    "type": texto("grant"), "amount": inteiro(FICHAS_INICIAIS), "reason": texto("initial_grant"),
                    "balanceAfter": inteiro(FICHAS_INICIAIS), "createdAt": data(agora),
                } },
                "currentDocument": { "exists": false },
            }));
        }
        escritas.push(json!({
            "update": { "name": fs.nome(&carteira_caminho), "fields": campos_carteira },
            "updateMask": { "fieldPaths": mascara },
        }));
        escritas.push(json!({
            "update": { "name": fs.nome(&lancamento), "fields": {
                "type": texto("grant"), "amount": inteiro(quantidade), "reason": texto("admin_grant"),
                "grantId": texto(id), "grantReason": texto(motivo),
                "balanceAfter": inteiro(saldo_final), "createdAt": data(agora),
            } },
            "currentDocument": { "exists": false },
        }));
        fs.confirmar(&tx, escritas).await?;
        Ok(true)
    }
    .await;
    if !matches!(resultado, Ok(true)) {
        fs.desfazer(&tx).await;
    }
    resultado
}

/// Credita, tentando de novo quando a carteira mudou no meio (o jogador iniciou uma run).
async fn creditar_com_novas_tentativas(fs: &Firestore, uid: &str, id: &str, quantidade: i64, motivo: &str) -> Result<bool, AppError> {
    let mut tentativa = 0;
    loop {
        match creditar(fs, uid, id, quantidade, motivo, Utc::now()).await {
            Err(AppError::Conflict(_)) if tentativa < 4 => {
                tentativa += 1;
                tokio::time::sleep(std::time::Duration::from_millis(150 * tentativa)).await;
            }
            outro => return outro,
        }
    }
}

/// Credita a distribuição (se venceu e ninguém está creditando). Quem já recebeu é pulado.
pub async fn executar(fs: &Firestore, id: &str) -> Result<(), AppError> {
    let Some(campos) = ocupar(fs, id, Utc::now()).await? else { return Ok(()) };
    let distribuicao = distribuicao_de(id.to_string(), &campos);
    let destinatarios = match distribuicao.target.as_str() {
        "all" => todos_os_jogadores(fs).await,
        _ => Ok(distribuicao.uid.clone().into_iter().collect()),
    };
    let (creditados, falhas) = match destinatarios {
        Err(e) => (0, vec![format!("{e:?}")]),
        Ok(uids) => creditar_todos(fs, &uids, &distribuicao).await,
    };

    let mut fim = Campos::new();
    fim.insert("executedAt".into(), data(Utc::now()));
    fim.insert("recipients".into(), inteiro(creditados));
    let status = if falhas.is_empty() { "done" } else { "failed" };
    fim.insert("status".into(), texto(status));
    // `error` na máscara sem valor apaga o erro de uma tentativa anterior.
    if !falhas.is_empty() {
        let resumo = format!("{} jogador(es) sem crédito; use Repetir. {}", falhas.len(), falhas.first().cloned().unwrap_or_default());
        fim.insert("error".into(), texto(&resumo.chars().take(400).collect::<String>()));
    }
    fs.gravar(&caminho(id), fim, &["executedAt", "recipients", "status", "error"]).await?;
    println!("fichas: distribuição {id} ({} x {}) {status}: {creditados} creditados, {} falhas", distribuicao.target, distribuicao.amount, falhas.len());
    Ok(())
}

/// Credita uma lista de jogadores, alguns em paralelo. Devolve (creditados, falhas).
async fn creditar_todos(fs: &Firestore, uids: &[String], d: &Distribuicao) -> (i64, Vec<String>) {
    let mut creditados = 0;
    let mut falhas = Vec::new();
    for lote in uids.chunks(CREDITOS_EM_PARALELO) {
        let tarefas = lote.iter().map(|uid| async move { (uid, creditar_com_novas_tentativas(fs, uid, &d.id, d.amount, &d.reason).await) });
        for (uid, resultado) in juntar(tarefas).await {
            match resultado {
                // Já recebido numa tentativa anterior também conta: ele tem as fichas.
                Ok(_) => creditados += 1,
                Err(e) => falhas.push(format!("{uid}: {e:?}")),
            }
        }
    }
    (creditados, falhas)
}

/// Roda os futuros juntos (na mesma tarefa) e devolve os resultados na ordem.
async fn juntar<F: std::future::Future>(futuros: impl Iterator<Item = F>) -> Vec<F::Output> {
    let mut pendentes: Vec<std::pin::Pin<Box<F>>> = futuros.map(Box::pin).collect();
    let mut saidas: Vec<Option<F::Output>> = (0..pendentes.len()).map(|_| None).collect();
    std::future::poll_fn(|cx| {
        let mut faltam = false;
        for (i, futuro) in pendentes.iter_mut().enumerate() {
            if saidas[i].is_some() {
                continue;
            }
            match futuro.as_mut().poll(cx) {
                std::task::Poll::Ready(saida) => saidas[i] = Some(saida),
                std::task::Poll::Pending => faltam = true,
            }
        }
        if faltam { std::task::Poll::Pending } else { std::task::Poll::Ready(()) }
    })
    .await;
    saidas.into_iter().map(|s| s.expect("todos os futuros terminaram")).collect()
}

/// Credita as distribuições vencidas (e retoma as travadas).
pub async fn processar_vencidas(fs: &Firestore) -> Result<usize, AppError> {
    let agora = Utc::now();
    let pendentes = fs
        .consultar(json!({
            "from": [{ "collectionId": COLECAO }],
            "where": { "fieldFilter": { "field": { "fieldPath": "status" }, "op": "IN", "value": { "arrayValue": { "values": [texto("scheduled"), texto("running")] } } } },
        }))
        .await?;
    let mut feitas = 0;
    for (id, campos) in pendentes {
        let vencida = match ler_texto(&campos, "status").as_deref() {
            Some("scheduled") => ler_data(&campos, "scheduledAt").is_some_and(|q| q <= agora),
            _ => ler_data(&campos, "claimedAt").is_none_or(|q| q < agora - Duration::minutes(TRAVADA_APOS_MINUTOS)),
        };
        if vencida {
            executar(fs, &id).await?;
            feitas += 1;
        }
    }
    Ok(feitas)
}

/// Chamado em toda requisição: no máximo a cada [`INTERVALO_DE_CONFERENCIA`] segundos por instância,
/// confere e credita as distribuições vencidas. Roda junto com a requisição (o Cloud Run só dá CPU
/// enquanto atende), sem atrasá-la quando não há nada vencido além de uma consulta curta.
pub async fn acordar(fs: Arc<Firestore>) {
    static ULTIMA: AtomicI64 = AtomicI64::new(0);
    static OCUPADO: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let agora = Utc::now().timestamp();
    let ultima = ULTIMA.load(Ordering::Relaxed);
    if agora - ultima < INTERVALO_DE_CONFERENCIA || ULTIMA.compare_exchange(ultima, agora, Ordering::Relaxed, Ordering::Relaxed).is_err() {
        return;
    }
    let Ok(_vez) = OCUPADO.try_lock() else { return };
    if let Err(e) = processar_vencidas(&fs).await {
        eprintln!("fichas: falha ao conferir as distribuições agendadas: {e:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entrada(target: &str, uid: Option<&str>, amount: i64, reason: &str, note: Option<&str>) -> DistribuicaoEntrada {
        DistribuicaoEntrada {
            target: target.into(),
            uid: uid.map(str::to_string),
            amount,
            reason: reason.into(),
            note: note.map(str::to_string),
            scheduled_at: None,
        }
    }

    #[test]
    fn valida_destino_quantidade_e_motivo() {
        let agora = Utc::now();
        assert!(validar(&entrada("all", None, 10, "event_prize", None), agora).is_ok());
        assert!(validar(&entrada("user", Some("abc123"), 5, "compensation", Some(" queda do servidor ")), agora).is_ok());
        assert!(validar(&entrada("all", Some("abc"), 10, "event_prize", None), agora).is_err());
        assert!(validar(&entrada("user", None, 10, "event_prize", None), agora).is_err());
        assert!(validar(&entrada("user", Some("../wallets/x"), 10, "event_prize", None), agora).is_err());
        assert!(validar(&entrada("todos", None, 10, "event_prize", None), agora).is_err());
        assert!(validar(&entrada("all", None, 0, "event_prize", None), agora).is_err());
        assert!(validar(&entrada("all", None, MAXIMO_POR_JOGADOR + 1, "event_prize", None), agora).is_err());
        assert!(validar(&entrada("all", None, 10, "bonus", None), agora).is_err());
        // "Outro" precisa de observação.
        assert!(validar(&entrada("all", None, 10, "other", Some("  ")), agora).is_err());
        assert!(validar(&entrada("all", None, 10, "other", Some("aniversário do ShmupX")), agora).is_ok());
    }

    #[test]
    fn horario_do_agendamento() {
        let agora = Utc::now();
        let mut e = entrada("all", None, 1, "promotion", None);
        assert_eq!(validar(&e, agora).unwrap().1, agora);
        e.scheduled_at = Some(agora + Duration::hours(3));
        assert_eq!(validar(&e, agora).unwrap().1, agora + Duration::hours(3));
        // Um pouco no passado (relógio do PC) vira agora; muito no passado é recusado.
        e.scheduled_at = Some(agora - Duration::minutes(2));
        assert_eq!(validar(&e, agora).unwrap().1, agora);
        e.scheduled_at = Some(agora - Duration::hours(1));
        assert!(validar(&e, agora).is_err());
        e.scheduled_at = Some(agora + Duration::days(400));
        assert!(validar(&e, agora).is_err());
    }

    #[test]
    fn observacao_limpa_e_limitada() {
        let agora = Utc::now();
        let (nota, _) = validar(&entrada("all", None, 1, "promotion", Some("  Black Friday  ")), agora).unwrap();
        assert_eq!(nota.as_deref(), Some("Black Friday"));
        let longa = "x".repeat(201);
        assert!(validar(&entrada("all", None, 1, "promotion", Some(&longa)), agora).is_err());
    }

    async fn saldo(fs: &Firestore, uid: &str) -> Option<i64> {
        fs.ler_documento(&format!("wallets/{uid}")).await.unwrap().and_then(|c| ler_inteiro(&c, "balance"))
    }

    #[tokio::test]
    #[ignore = "exige o emulador do Firestore"]
    async fn distribuicoes_creditam_uma_vez_so_no_emulador() {
        // SAFETY: teste isolado com --ignored; nenhuma outra thread lê estas variáveis agora.
        unsafe { std::env::set_var("FIREBASE_PROJECT_ID", "demo-shmupx") };
        let fs = Firestore::do_ambiente().unwrap();
        let (ana, beto, banido) = (format!("ana-{}", Uuid::new_v4()), format!("beto-{}", Uuid::new_v4()), format!("ban-{}", Uuid::new_v4()));
        let agora = Utc::now();
        for uid in [&ana, &beto, &banido] {
            let mut c = Campos::new();
            c.insert("displayName".into(), texto(uid));
            c.insert("createdAt".into(), data(agora));
            fs.gravar_novo(&format!("users/{uid}"), c).await.unwrap();
        }
        // Ana já tem carteira com 7 fichas; Beto ainda não jogou (sem carteira).
        let mut carteira = Campos::new();
        carteira.insert("balance".into(), inteiro(7));
        carteira.insert("version".into(), inteiro(3));
        fs.gravar_novo(&format!("wallets/{ana}"), carteira).await.unwrap();
        let mut ban = Campos::new();
        ban.insert("banned".into(), crate::firestore::valor::booleano(true));
        fs.gravar_novo(&format!("moderation/{banido}"), ban).await.unwrap();

        // Para um jogador, agora: credita na hora.
        let feita = criar(&fs, DistribuicaoEntrada { target: "user".into(), uid: Some(ana.clone()), amount: 5, reason: "compensation".into(), note: None, scheduled_at: None }, "admin@x").await.unwrap();
        assert_eq!((feita.status.as_str(), feita.recipients), ("done", 1));
        assert_eq!(feita.user_label.as_deref(), Some(ana.as_str()));
        assert_eq!(saldo(&fs, &ana).await, Some(12));

        // Para todos, agendada: só credita quando vence; banido fica de fora; carteira nova nasce com as iniciais.
        let futura = criar(&fs, DistribuicaoEntrada { target: "all".into(), uid: None, amount: 3, reason: "event_prize".into(), note: None, scheduled_at: Some(agora + Duration::hours(1)) }, "admin@x").await.unwrap();
        assert_eq!(futura.status, "scheduled");
        processar_vencidas(&fs).await.unwrap();
        assert_eq!(saldo(&fs, &ana).await, Some(12), "ainda não venceu");
        let mut vencer = Campos::new();
        vencer.insert("scheduledAt".into(), data(agora - Duration::minutes(1)));
        fs.gravar(&caminho(&futura.id), vencer, &["scheduledAt"]).await.unwrap();
        processar_vencidas(&fs).await.unwrap();
        let feita = obter(&fs, &futura.id).await.unwrap();
        assert_eq!(feita.status, "done");
        assert!(feita.recipients >= 2);
        assert_eq!(saldo(&fs, &ana).await, Some(15));
        assert_eq!(saldo(&fs, &beto).await, Some(FICHAS_INICIAIS + 3));
        assert_eq!(saldo(&fs, &banido).await, None, "banido não recebe");

        // Repetir (falhou no meio) não credita de novo quem já recebeu.
        let mut falhou = Campos::new();
        falhou.insert("status".into(), texto("failed"));
        fs.gravar(&caminho(&futura.id), falhou, &["status"]).await.unwrap();
        assert_eq!(repetir(&fs, &futura.id).await.unwrap().status, "done");
        assert_eq!(saldo(&fs, &ana).await, Some(15));
        assert_eq!(saldo(&fs, &beto).await, Some(FICHAS_INICIAIS + 3));

        // Cancelar só agendada.
        let outra = criar(&fs, DistribuicaoEntrada { target: "user".into(), uid: Some(beto.clone()), amount: 1, reason: "promotion".into(), note: None, scheduled_at: Some(agora + Duration::days(1)) }, "admin@x").await.unwrap();
        assert_eq!(cancelar(&fs, &outra.id).await.unwrap().status, "cancelled");
        assert!(cancelar(&fs, &outra.id).await.is_err());
        assert!(cancelar(&fs, &futura.id).await.is_err());
        // Jogador inexistente.
        assert!(criar(&fs, DistribuicaoEntrada { target: "user".into(), uid: Some("ninguem-123".into()), amount: 1, reason: "promotion".into(), note: None, scheduled_at: None }, "admin@x").await.is_err());
    }

    #[tokio::test]
    async fn juntar_devolve_na_ordem() {
        let saidas = juntar([3u64, 1, 2].into_iter().map(|n| async move {
            tokio::time::sleep(std::time::Duration::from_millis(n * 5)).await;
            n
        }))
        .await;
        assert_eq!(saidas, vec![3, 1, 2]);
    }
}
