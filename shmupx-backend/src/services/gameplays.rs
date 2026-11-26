//! Fichas e gameplays no Firestore (Spec 002, ADR 005).
//!
//! - `iniciar`: numa transação, valida o jogo publicado e o saldo, debita 1 ficha em
//!   `wallets/{uid}` com lançamento imutável em `wallets/{uid}/entries`, e abre
//!   `gameplays/{runId}` em `recording`. Carteira inexistente nasce com `FICHAS_INICIAIS`.
//! - `encerrar`: no STOP (ou no corte da ficha), registra o fim da captura pelo relógio do
//!   servidor e confere que a gameplay coube em `serverStartedAt + maxDurationSeconds +
//!   tolerância`. O estado vai para `processing`: gravação encerrada, análise pendente.
//! - `submeter`: grava o resultado do validador offline na gameplay do próprio jogador. O limite
//!   de tempo é da **gameplay**, não da análise: uma run em `processing` aceita o resultado a
//!   qualquer momento. Uma run ainda em `recording` (cliente antigo, ou o aviso de encerramento
//!   se perdeu) segue a janela da gameplay. O estado vai para `needs_review`: completar a análise
//!   não aprova a gameplay (README do Firebase); aprovação e publicação de ranking são etapas
//!   posteriores do backend.

use chrono::{DateTime, Duration as ChronoDuration, SubsecRound, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::AppError;
use crate::firestore::valor::{
    booleano, data, inteiro, ler_booleano, ler_data, ler_inteiro, ler_lista_de_textos, ler_mapa, ler_texto, mapa, texto,
};
use crate::firestore::{Campos, Firestore};

/// Saldo de uma carteira criada no primeiro início de run.
pub const FICHAS_INICIAIS: i64 = 50;

/// Duração usada quando o jogo não define `maxDurationSeconds` (Spec 002: 45 a 60 min).
const DURACAO_PADRAO_SEGUNDOS: i64 = 3600;

/// Margem sobre a duração máxima para a latência entre o fim da gameplay (STOP ou corte da
/// ficha) e o aviso ao servidor (Spec 002).
pub const TOLERANCIA_ENVIO_SEGUNDOS: i64 = 120;

#[derive(Debug, Serialize, PartialEq)]
pub struct RunIniciada {
    pub run_id: String,
    pub server_started_at: DateTime<Utc>,
    pub max_duration_segundos: i32,
}

/// Vidas e continues são opcionais: o MiSTer só lê o placar (ADR 008).
#[derive(Debug, Clone, PartialEq)]
pub struct Resultado {
    pub score_bruto: i64,
    pub vidas_perdidas: Option<i32>,
    pub continues_usados: Option<i32>,
    /// `true` quando o jogador zerou o jogo (o fim do jogo, não só o game over). Com 0 continues, a
    /// run de Hi-Score entra também na tabela 1CC.
    pub zerou: Option<bool>,
}

/// Como a partida foi jogada. Ranking: mesmo jogo e mesma versão, venha de PC, emulador,
/// Steam, MiSTer ou placa original.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Detalhes {
    /// `highscore`, `1cc`, `caravan-sprint` (2 min) ou `caravan-classic` (5 min).
    pub modo: Option<String>,
    /// Versão do jogo (ex.: `megadrive-4.0.0`); partidas de versões diferentes não se misturam.
    pub versao: Option<String>,
    /// `pc` ou `mister`, pela credencial usada (`middleware::jogador`).
    pub origem: &'static str,
    /// Id da live no YouTube que transmitiu a partida (Ranked com live). Se a run entrar no top 3,
    /// a live vai para o canal oficial da ShmupX (`services::destaques`).
    pub live_broadcast_id: Option<String>,
    /// Evento em que a partida vale (o jogador se inscreveu pagando as fichas do evento). A run
    /// continua gastando 1 ficha e entra no ranking; o evento guarda a run pelo `eventId`.
    pub evento_id: Option<String>,
}

/// Modos de partida e o modo do catálogo (`games.modes`) a que pertencem. No 1CC a partida usa
/// uma ficha só: não há continue com outra ficha.
const MODOS_DE_PARTIDA: [(&str, &str); 4] =
    [("highscore", "highscore"), ("1cc", "1cc"), ("caravan-sprint", "caravan"), ("caravan-classic", "caravan")];

fn versao_valida(versao: &str) -> bool {
    (1..=40).contains(&versao.len()) && versao.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

#[derive(Debug, Serialize, PartialEq)]
pub struct RunEncerrada {
    pub run_id: String,
    pub status: &'static str,
    pub server_ended_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct RunSubmetida {
    pub run_id: String,
    pub status: &'static str,
}

/// IDs de documento vêm do cliente; só aceitamos o formato que nós mesmos geramos/publicamos.
pub(crate) fn id_valido(id: &str) -> bool {
    (1..=128).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub async fn iniciar(fs: &Firestore, uid: &str, game_id: &str, detalhes: &Detalhes) -> Result<RunIniciada, AppError> {
    if !id_valido(game_id) {
        return Err(AppError::bad_request("jogo_id inválido"));
    }
    if let Some(modo) = &detalhes.modo
        && !MODOS_DE_PARTIDA.iter().any(|(m, _)| m == modo)
    {
        return Err(AppError::bad_request("modo inválido: use highscore, 1cc, caravan-sprint ou caravan-classic"));
    }
    if detalhes.versao.as_deref().is_some_and(|v| !versao_valida(v)) {
        return Err(AppError::bad_request("versão do jogo inválida"));
    }
    if detalhes.live_broadcast_id.as_deref().is_some_and(|id| !crate::services::youtube::id_valido(id)) {
        return Err(AppError::bad_request("id da live inválido"));
    }
    if detalhes.evento_id.as_deref().is_some_and(|id| !id_valido(id)) {
        return Err(AppError::bad_request("id do evento inválido"));
    }
    let tx = fs.iniciar_transacao().await?;
    let resultado = iniciar_na_transacao(fs, &tx, uid, game_id, detalhes).await;
    if resultado.is_err() {
        fs.desfazer(&tx).await;
    }
    resultado
}

async fn iniciar_na_transacao(
    fs: &Firestore,
    tx: &str,
    uid: &str,
    game_id: &str,
    detalhes: &Detalhes,
) -> Result<RunIniciada, AppError> {
    let jogo = fs
        .ler(&format!("games/{game_id}"), tx)
        .await?
        .filter(|jogo| ler_booleano(jogo, "published") == Some(true))
        .ok_or_else(|| AppError::not_found("jogo não encontrado"))?;
    // No MiSTer, Ranked só nos jogos com leitura de placar pela memória (flag do Lab).
    if detalhes.origem == "mister" && ler_booleano(&jogo, "rankedMister") != Some(true) {
        return Err(AppError::bad_request("Ranked no MiSTer não está liberado para este jogo: jogue em Training"));
    }
    // Jogo com modos no catálogo só aceita partidas desses modos.
    let modos_do_jogo = ler_lista_de_textos(&jogo, "modes");
    if let Some(modo) = &detalhes.modo {
        let categoria = MODOS_DE_PARTIDA.iter().find(|(m, _)| m == modo).map(|(_, c)| *c).unwrap_or_default();
        if !modos_do_jogo.is_empty() && !modos_do_jogo.iter().any(|m| m == categoria) {
            return Err(AppError::bad_request(format!("este jogo não tem o modo {categoria}")));
        }
    }
    let max_duracao = ler_inteiro(&jogo, "maxDurationSeconds").unwrap_or(DURACAO_PADRAO_SEGUNDOS).clamp(1, 7200);

    // Partida de evento: só para quem se inscreveu, durante o evento, num jogo e modo do evento.
    let agora = Utc::now();
    if let Some(evento_id) = &detalhes.evento_id {
        crate::services::eventos::exigir_partida_de_evento(fs, tx, uid, evento_id, game_id, detalhes.modo.as_deref(), agora).await?;
    }

    let run_id = Uuid::new_v4().to_string();
    let carteira = fs.ler(&format!("wallets/{uid}"), tx).await?;
    let (mut escritas, _) = debito_na_carteira(fs, uid, carteira.as_ref(), 1, "run_start", ("runId", &run_id), agora)?;
    let mut campos_gameplay = campos(&[
        ("ownerId", texto(uid)),
        ("gameId", texto(game_id)),
        ("source", texto(detalhes.origem)),
        ("status", texto("recording")),
        ("createdAt", data(agora)),
        ("updatedAt", data(agora)),
        ("capture", mapa(campos(&[
            ("serverStartedAt", data(agora)),
            ("maxDurationSeconds", inteiro(max_duracao)),
        ]))),
    ]);
    if let Some(modo) = &detalhes.modo {
        campos_gameplay.insert("mode".into(), texto(modo));
    }
    if let Some(versao) = &detalhes.versao {
        campos_gameplay.insert("gameVersion".into(), texto(versao));
    }
    if let Some(evento_id) = &detalhes.evento_id {
        campos_gameplay.insert("eventId".into(), texto(evento_id));
    }
    if let Some(id) = &detalhes.live_broadcast_id {
        campos_gameplay.insert("live".into(), mapa(campos(&[
            ("broadcastId", texto(id)),
            ("watchUrl", texto(&format!("https://www.youtube.com/watch?v={id}"))),
        ])));
    }
    escritas.push(criar(fs, &format!("gameplays/{run_id}"), campos_gameplay));

    fs.confirmar(tx, escritas).await?;
    Ok(RunIniciada { run_id, server_started_at: agora, max_duration_segundos: max_duracao as i32 })
}

/// Escritas que debitam `valor` fichas da carteira (`wallets/{uid}`, já lida na transação) com o
/// lançamento imutável em `entries` (`motivo` e a referência, ex.: `("runId", id)`), e o saldo que
/// sobra. Carteira inexistente nasce com `FICHAS_INICIAIS` (e o lançamento do bônus inicial).
/// Saldo menor que `valor`: recusa sem escrever nada.
pub(crate) fn debito_na_carteira(
    fs: &Firestore,
    uid: &str,
    carteira: Option<&Campos>,
    valor: i64,
    motivo: &str,
    referencia: (&str, &str),
    agora: DateTime<Utc>,
) -> Result<(Vec<Value>, i64), AppError> {
    let caminho = format!("wallets/{uid}");
    let saldo = carteira.and_then(|c| ler_inteiro(c, "balance")).unwrap_or(FICHAS_INICIAIS);
    let versao = carteira.and_then(|c| ler_inteiro(c, "version")).unwrap_or(0);
    if saldo < valor.max(1) {
        return Err(AppError::bad_request("saldo de fichas insuficiente"));
    }
    let saldo_final = saldo - valor;
    let mut escritas = Vec::new();
    let mut campos_carteira = campos(&[("balance", inteiro(saldo_final)), ("version", inteiro(versao + 1)), ("updatedAt", data(agora))]);
    if carteira.is_none() {
        campos_carteira.insert("createdAt".into(), data(agora));
        escritas.push(criar(fs, &format!("{caminho}/entries/{}", Uuid::new_v4()), campos(&[
            ("type", texto("grant")),
            ("amount", inteiro(FICHAS_INICIAIS)),
            ("reason", texto("initial_grant")),
            ("balanceAfter", inteiro(FICHAS_INICIAIS)),
            ("createdAt", data(agora)),
        ])));
    }
    escritas.push(atualizar(fs, &caminho, campos_carteira, None));
    escritas.push(criar(fs, &format!("{caminho}/entries/{}", Uuid::new_v4()), campos(&[
        ("type", texto("debit")),
        ("amount", inteiro(-valor)),
        ("reason", texto(motivo)),
        (referencia.0, texto(referencia.1)),
        ("balanceAfter", inteiro(saldo_final)),
        ("createdAt", data(agora)),
    ])));
    Ok((escritas, saldo_final))
}

/// A gameplay (do início ao aviso de encerramento) coube no tempo da ficha, com a tolerância.
fn dentro_do_tempo(captura: &Campos, agora: DateTime<Utc>) -> Result<bool, AppError> {
    let inicio = ler_data(captura, "serverStartedAt").ok_or_else(|| AppError::internal("gameplay sem serverStartedAt"))?;
    let max_duracao = ler_inteiro(captura, "maxDurationSeconds").unwrap_or(DURACAO_PADRAO_SEGUNDOS);
    Ok(agora - inicio <= ChronoDuration::seconds(max_duracao + TOLERANCIA_ENVIO_SEGUNDOS))
}

/// Encerra a captura da run: o servidor registra o fim pelo próprio relógio. Repetir o aviso de
/// uma run já encerrada é idempotente.
pub async fn encerrar(fs: &Firestore, uid: &str, run_id: &str) -> Result<RunEncerrada, AppError> {
    if !id_valido(run_id) {
        return Err(AppError::bad_request("run_id inválido"));
    }
    let tx = fs.iniciar_transacao().await?;
    let saida = encerrar_na_transacao(fs, &tx, uid, run_id).await;
    if matches!(saida, Err(_) | Ok(Encerramento::JaEncerrada(_))) {
        fs.desfazer(&tx).await;
    }
    match saida? {
        Encerramento::Encerrada(run, jogo, segundos) => {
            crate::services::estatisticas::run_terminou(fs, uid, &jogo, segundos).await;
            Ok(run)
        }
        Encerramento::JaEncerrada(run) => Ok(run),
        Encerramento::Excedeu(jogo, segundos) => {
            crate::services::estatisticas::run_terminou(fs, uid, &jogo, segundos).await;
            Err(AppError::conflict("a gameplay passou do tempo da ficha; a run expirou"))
        }
    }
}

/// Como a run terminou; nos fins novos, o jogo e os segundos jogados (para os totais do jogador).
enum Encerramento {
    Encerrada(RunEncerrada, String, i64),
    JaEncerrada(RunEncerrada),
    Excedeu(String, i64),
}

async fn encerrar_na_transacao(fs: &Firestore, tx: &str, uid: &str, run_id: &str) -> Result<Encerramento, AppError> {
    let caminho = format!("gameplays/{run_id}");
    let gameplay = fs
        .ler(&caminho, tx)
        .await?
        .filter(|g| ler_texto(g, "ownerId").as_deref() == Some(uid))
        .ok_or_else(|| AppError::not_found("run não encontrada"))?;
    let captura = ler_mapa(&gameplay, "capture").unwrap_or_default();

    match ler_texto(&gameplay, "status").as_deref() {
        Some("recording") => {}
        Some("processing") => {
            let fim = ler_data(&captura, "serverEndedAt").ok_or_else(|| AppError::internal("gameplay sem serverEndedAt"))?;
            return Ok(Encerramento::JaEncerrada(RunEncerrada { run_id: run_id.to_string(), status: "processing", server_ended_at: fim }));
        }
        _ => return Err(AppError::conflict("esta run já foi encerrada")),
    }

    // Microssegundos, a precisão que o Firestore guarda: o valor devolvido bate com o gravado.
    let agora = Utc::now().trunc_subsecs(6);
    let jogo = ler_texto(&gameplay, "gameId").unwrap_or_default();
    let max_duracao = ler_inteiro(&captura, "maxDurationSeconds").unwrap_or(DURACAO_PADRAO_SEGUNDOS);
    let segundos = ler_data(&captura, "serverStartedAt").map(|inicio| (agora - inicio).num_seconds().clamp(0, max_duracao)).unwrap_or(0);
    if !dentro_do_tempo(&captura, agora)? {
        let escrita = atualizar(fs, &caminho, campos(&[("status", texto("expired")), ("updatedAt", data(agora))]), Some(&["status", "updatedAt"]));
        fs.confirmar(tx, vec![escrita]).await?;
        return Ok(Encerramento::Excedeu(jogo, segundos));
    }
    let escrita = atualizar(
        fs,
        &caminho,
        campos(&[
            ("status", texto("processing")),
            ("updatedAt", data(agora)),
            ("capture", mapa(campos(&[("serverEndedAt", data(agora))]))),
        ]),
        Some(&["status", "updatedAt", "capture.serverEndedAt"]),
    );
    fs.confirmar(tx, vec![escrita]).await?;
    Ok(Encerramento::Encerrada(RunEncerrada { run_id: run_id.to_string(), status: "processing", server_ended_at: agora }, jogo, segundos))
}

pub async fn submeter(
    fs: &Firestore,
    uid: &str,
    run_id: &str,
    game_id: &str,
    resultado: Resultado,
) -> Result<RunSubmetida, AppError> {
    if resultado.score_bruto < 0
        || resultado.vidas_perdidas.is_some_and(|v| v < 0)
        || resultado.continues_usados.is_some_and(|c| c < 0)
    {
        return Err(AppError::bad_request("score_bruto, vidas_perdidas e continues_usados não podem ser negativos"));
    }
    if !id_valido(run_id) {
        return Err(AppError::bad_request("run_id inválido"));
    }
    let tx = fs.iniciar_transacao().await?;
    let saida = submeter_na_transacao(fs, &tx, uid, run_id, game_id, &resultado).await;
    // Só desfaz o que não chegou a ser confirmado.
    if matches!(saida, Err(_) | Ok(Saida::JaRegistrado(_))) {
        fs.desfazer(&tx).await;
    }
    match saida? {
        Saida::Gravado(run) | Saida::JaRegistrado(run) => Ok(run),
        Saida::Expirada => Err(AppError::conflict("janela de envio expirada para esta run; a ficha não é devolvida")),
    }
}

enum Saida {
    Gravado(RunSubmetida),
    /// Reenvio idêntico (ex.: a resposta anterior se perdeu na rede): sucesso sem nova escrita.
    JaRegistrado(RunSubmetida),
    Expirada,
}

async fn submeter_na_transacao(
    fs: &Firestore,
    tx: &str,
    uid: &str,
    run_id: &str,
    game_id: &str,
    resultado: &Resultado,
) -> Result<Saida, AppError> {
    let caminho = format!("gameplays/{run_id}");
    // Gameplay de outro jogador responde como inexistente: não revela IDs alheios.
    let gameplay = fs
        .ler(&caminho, tx)
        .await?
        .filter(|g| ler_texto(g, "ownerId").as_deref() == Some(uid))
        .ok_or_else(|| AppError::not_found("run não encontrada"))?;
    if ler_texto(&gameplay, "gameId").as_deref() != Some(game_id) {
        return Err(AppError::bad_request("a run pertence a outro jogo"));
    }

    let submetida = RunSubmetida { run_id: run_id.to_string(), status: "needs_review" };
    let captura = ler_mapa(&gameplay, "capture").unwrap_or_default();
    let agora = Utc::now();
    match ler_texto(&gameplay, "status").as_deref() {
        // Captura encerrada no servidor: a análise pode levar o tempo que precisar.
        Some("processing") => {}
        // Sem aviso de encerramento, o envio é o único marco: vale a janela da gameplay.
        Some("recording") => {
            if !dentro_do_tempo(&captura, agora)? {
                let escrita = atualizar(fs, &caminho, campos(&[("status", texto("expired")), ("updatedAt", data(agora))]), Some(&["status", "updatedAt"]));
                fs.confirmar(tx, vec![escrita]).await?;
                return Ok(Saida::Expirada);
            }
        }
        Some("needs_review") if ler_mapa(&gameplay, "result").as_ref().map(ler_resultado) == Some(Some(resultado.clone())) => {
            return Ok(Saida::JaRegistrado(submetida));
        }
        _ => return Err(AppError::conflict("esta run já foi encerrada")),
    }

    let mut campos_resultado = campos(&[("score", inteiro(resultado.score_bruto)), ("submittedAt", data(agora))]);
    if let Some(vidas) = resultado.vidas_perdidas {
        campos_resultado.insert("livesLost".into(), inteiro(vidas.into()));
    }
    if let Some(continues) = resultado.continues_usados {
        campos_resultado.insert("continuesUsed".into(), inteiro(continues.into()));
    }
    if let Some(zerou) = resultado.zerou {
        campos_resultado.insert("cleared".into(), booleano(zerou));
    }
    let escrita = atualizar(
        fs,
        &caminho,
        campos(&[("status", texto("needs_review")), ("updatedAt", data(agora)), ("result", mapa(campos_resultado))]),
        Some(&["status", "updatedAt", "result"]),
    );
    fs.confirmar(tx, vec![escrita]).await?;
    Ok(Saida::Gravado(submetida))
}

fn ler_resultado(campos: &Campos) -> Option<Resultado> {
    Some(Resultado {
        score_bruto: ler_inteiro(campos, "score")?,
        vidas_perdidas: ler_inteiro(campos, "livesLost").and_then(|v| v.try_into().ok()),
        continues_usados: ler_inteiro(campos, "continuesUsed").and_then(|v| v.try_into().ok()),
        zerou: ler_booleano(campos, "cleared"),
    })
}

fn campos(pares: &[(&str, Value)]) -> Campos {
    pares.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

/// Cria o documento; falha se ele já existir (IDs novos nunca colidem com dados anteriores).
pub(crate) fn criar(fs: &Firestore, caminho: &str, campos: Campos) -> Value {
    json!({ "update": { "name": fs.nome(caminho), "fields": campos }, "currentDocument": { "exists": false } })
}

/// Sem máscara, substitui o documento inteiro; com máscara, altera só os campos listados.
fn atualizar(fs: &Firestore, caminho: &str, campos: Campos, mascara: Option<&[&str]>) -> Value {
    let mut escrita = json!({ "update": { "name": fs.nome(caminho), "fields": campos } });
    if let Some(mascara) = mascara {
        escrita["updateMask"] = json!({ "fieldPaths": mascara });
    }
    escrita
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_aceitos_sao_os_gerados_pelo_sistema() {
        assert!(id_valido("earthion"));
        assert!(id_valido(&Uuid::new_v4().to_string()));
        assert!(!id_valido(""));
        assert!(!id_valido("../wallets/outro"));
        assert!(!id_valido("a/b"));
        assert!(!id_valido(&"x".repeat(129)));
    }

    /// Fluxo completo contra o emulador (nunca contra produção). Rodar a partir de
    /// `shmupx-infra/firebase`:
    /// `npx firebase-tools emulators:exec --only firestore --project demo-shmupx "cd ../../shmupx-backend && cargo test -- --ignored"`
    #[tokio::test]
    #[ignore = "exige o emulador do Firestore"]
    async fn fluxo_de_fichas_e_gameplay_no_emulador() {
        let Ok(host) = std::env::var("FIRESTORE_EMULATOR_HOST") else {
            panic!("FIRESTORE_EMULATOR_HOST ausente: este teste só roda no emulador");
        };
        // SAFETY: teste isolado com --ignored; nenhuma outra thread lê estas variáveis agora.
        unsafe { std::env::set_var("FIREBASE_PROJECT_ID", "demo-shmupx") };
        let fs = Firestore::do_ambiente().unwrap();
        assert!(fs.nome("x").starts_with("projects/demo-shmupx/"), "emulador em {host}");

        let jogo = format!("jogo-{}", Uuid::new_v4());
        let oculto = format!("oculto-{}", Uuid::new_v4());
        let tx = fs.iniciar_transacao().await.unwrap();
        fs.confirmar(&tx, vec![
            criar(&fs, &format!("games/{jogo}"), campos(&[("published", json!({ "booleanValue": true })), ("maxDurationSeconds", inteiro(3600))])),
            criar(&fs, &format!("games/{oculto}"), campos(&[("published", json!({ "booleanValue": false }))])),
        ]).await.unwrap();

        let uid = format!("uid-{}", Uuid::new_v4());

        // Jogo não publicado não abre run nem cria carteira.
        assert!(iniciar(&fs, &uid, &oculto, &Detalhes::default()).await.is_err());

        let run = iniciar(&fs, &uid, &jogo, &Detalhes::default()).await.unwrap();
        assert_eq!(run.max_duration_segundos, 3600);
        let leitura = fs.iniciar_transacao().await.unwrap();
        let carteira = fs.ler(&format!("wallets/{uid}"), &leitura).await.unwrap().unwrap();
        assert_eq!(ler_inteiro(&carteira, "balance"), Some(FICHAS_INICIAIS - 1));
        let gameplay = fs.ler(&format!("gameplays/{}", run.run_id), &leitura).await.unwrap().unwrap();
        assert_eq!(ler_texto(&gameplay, "ownerId").as_deref(), Some(uid.as_str()));
        assert_eq!(ler_texto(&gameplay, "status").as_deref(), Some("recording"));
        fs.desfazer(&leitura).await;

        // Segunda run debita de novo, sem recriar o bônus inicial.
        let segunda = iniciar(&fs, &uid, &jogo, &Detalhes::default()).await.unwrap();
        let leitura = fs.iniciar_transacao().await.unwrap();
        let carteira = fs.ler(&format!("wallets/{uid}"), &leitura).await.unwrap().unwrap();
        assert_eq!(ler_inteiro(&carteira, "balance"), Some(FICHAS_INICIAIS - 2));
        assert_eq!(ler_inteiro(&carteira, "version"), Some(2));
        fs.desfazer(&leitura).await;

        let resultado = Resultado { score_bruto: 123_456, vidas_perdidas: Some(2), continues_usados: Some(0), zerou: None };

        // Outro jogador não enxerga nem submete a run.
        assert!(submeter(&fs, "uid-intruso", &run.run_id, &jogo, resultado.clone()).await.is_err());
        // Jogo divergente é recusado.
        assert!(submeter(&fs, &uid, &run.run_id, &oculto, resultado.clone()).await.is_err());

        let enviada = submeter(&fs, &uid, &run.run_id, &jogo, resultado.clone()).await.unwrap();
        assert_eq!(enviada.status, "needs_review");
        // Reenvio idêntico é idempotente; resultado diferente é recusado.
        assert!(submeter(&fs, &uid, &run.run_id, &jogo, resultado.clone()).await.is_ok());
        let alterado = Resultado { score_bruto: 999_999_999, ..resultado.clone() };
        assert!(submeter(&fs, &uid, &run.run_id, &jogo, alterado).await.is_err());

        let leitura = fs.iniciar_transacao().await.unwrap();
        let gameplay = fs.ler(&format!("gameplays/{}", run.run_id), &leitura).await.unwrap().unwrap();
        assert_eq!(ler_mapa(&gameplay, "result").as_ref().and_then(ler_resultado), Some(resultado));
        let intacta = fs.ler(&format!("gameplays/{}", segunda.run_id), &leitura).await.unwrap().unwrap();
        assert_eq!(ler_texto(&intacta, "status").as_deref(), Some("recording"));
        fs.desfazer(&leitura).await;
    }

    /// Gameplay criada direto no banco, com o início no passado (simula o relógio andando).
    async fn gameplay_antiga(fs: &Firestore, uid: &str, jogo: &str, status: &str, minutos_atras: i64) -> String {
        let run_id = Uuid::new_v4().to_string();
        let inicio = Utc::now() - ChronoDuration::minutes(minutos_atras);
        let mut captura = campos(&[("serverStartedAt", data(inicio)), ("maxDurationSeconds", inteiro(600))]);
        if status == "processing" {
            captura.insert("serverEndedAt".into(), data(inicio + ChronoDuration::minutes(9)));
        }
        let tx = fs.iniciar_transacao().await.unwrap();
        fs.confirmar(&tx, vec![criar(fs, &format!("gameplays/{run_id}"), campos(&[
            ("ownerId", texto(uid)),
            ("gameId", texto(jogo)),
            ("status", texto(status)),
            ("capture", mapa(captura)),
        ]))]).await.unwrap();
        run_id
    }

    async fn status_de(fs: &Firestore, run_id: &str) -> Option<String> {
        let leitura = fs.iniciar_transacao().await.unwrap();
        let gameplay = fs.ler(&format!("gameplays/{run_id}"), &leitura).await.unwrap().unwrap();
        fs.desfazer(&leitura).await;
        ler_texto(&gameplay, "status")
    }

    /// O limite é da gameplay: encerrar marca o fim pelo relógio do servidor e a análise pode
    /// demorar o quanto precisar; passar do tempo da ficha expira a run.
    #[tokio::test]
    #[ignore = "exige o emulador do Firestore"]
    async fn encerramento_separa_tempo_de_gameplay_do_tempo_de_analise_no_emulador() {
        // SAFETY: ver o teste acima.
        unsafe { std::env::set_var("FIREBASE_PROJECT_ID", "demo-shmupx") };
        let fs = Firestore::do_ambiente().unwrap();
        let jogo = format!("jogo-{}", Uuid::new_v4());
        let uid = format!("uid-{}", Uuid::new_v4());
        let tx = fs.iniciar_transacao().await.unwrap();
        fs.confirmar(&tx, vec![criar(&fs, &format!("games/{jogo}"), campos(&[("published", json!({ "booleanValue": true })), ("maxDurationSeconds", inteiro(600))]))]).await.unwrap();
        let resultado = Resultado { score_bruto: 9_180_800, vidas_perdidas: Some(0), continues_usados: Some(0), zerou: None };

        // Fluxo normal: encerrar (idempotente), depois enviar.
        let run = iniciar(&fs, &uid, &jogo, &Detalhes::default()).await.unwrap();
        assert!(encerrar(&fs, "uid-intruso", &run.run_id).await.is_err(), "outro jogador não encerra");
        let encerrada = encerrar(&fs, &uid, &run.run_id).await.unwrap();
        assert_eq!(encerrada.status, "processing");
        assert_eq!(encerrar(&fs, &uid, &run.run_id).await.unwrap().server_ended_at, encerrada.server_ended_at);
        assert_eq!(status_de(&fs, &run.run_id).await.as_deref(), Some("processing"));
        assert_eq!(submeter(&fs, &uid, &run.run_id, &jogo, resultado.clone()).await.unwrap().status, "needs_review");
        assert!(encerrar(&fs, &uid, &run.run_id).await.is_err(), "run com resultado não volta a encerrar");

        // Análise longa: a captura terminou no tempo, o resultado chega 3 horas depois e vale.
        let analise_longa = gameplay_antiga(&fs, &uid, &jogo, "processing", 180).await;
        assert_eq!(submeter(&fs, &uid, &analise_longa, &jogo, resultado.clone()).await.unwrap().status, "needs_review");

        // Gameplay que passou do tempo da ficha: o encerramento expira a run.
        let excedeu = gameplay_antiga(&fs, &uid, &jogo, "recording", 30).await;
        assert!(encerrar(&fs, &uid, &excedeu).await.is_err());
        assert_eq!(status_de(&fs, &excedeu).await.as_deref(), Some("expired"));

        // Sem aviso de encerramento, vale a janela da gameplay no envio.
        let sem_aviso = gameplay_antiga(&fs, &uid, &jogo, "recording", 30).await;
        assert!(submeter(&fs, &uid, &sem_aviso, &jogo, resultado).await.is_err());
        assert_eq!(status_de(&fs, &sem_aviso).await.as_deref(), Some("expired"));
    }

    #[tokio::test]
    #[ignore = "exige o emulador do Firestore"]
    async fn mister_so_abre_ranked_com_a_flag_no_emulador() {
        // SAFETY: ver o teste acima.
        unsafe { std::env::set_var("FIREBASE_PROJECT_ID", "demo-shmupx") };
        let fs = Firestore::do_ambiente().unwrap();
        let (sem, com) = (format!("jogo-{}", Uuid::new_v4()), format!("jogo-{}", Uuid::new_v4()));
        let tx = fs.iniciar_transacao().await.unwrap();
        fs.confirmar(&tx, vec![
            criar(&fs, &format!("games/{sem}"), campos(&[("published", json!({ "booleanValue": true }))])),
            criar(&fs, &format!("games/{com}"), campos(&[("published", json!({ "booleanValue": true })), ("rankedMister", json!({ "booleanValue": true }))])),
        ]).await.unwrap();
        let uid = format!("uid-{}", Uuid::new_v4());
        let mister = Detalhes { origem: "mister", ..Detalhes::default() };
        let erro = iniciar(&fs, &uid, &sem, &mister).await.unwrap_err();
        assert!(format!("{erro:?}").contains("Ranked no MiSTer"));
        assert!(iniciar(&fs, &uid, &com, &mister).await.is_ok());
        // No PC a flag do MiSTer não muda nada.
        assert!(iniciar(&fs, &uid, &sem, &Detalhes { origem: "pc", ..Detalhes::default() }).await.is_ok());
    }

    #[tokio::test]
    #[ignore = "exige o emulador do Firestore"]
    async fn saldo_zerado_recusa_nova_run_no_emulador() {
        // SAFETY: ver o teste acima.
        unsafe { std::env::set_var("FIREBASE_PROJECT_ID", "demo-shmupx") };
        let fs = Firestore::do_ambiente().unwrap();
        let jogo = format!("jogo-{}", Uuid::new_v4());
        let uid = format!("uid-{}", Uuid::new_v4());
        let tx = fs.iniciar_transacao().await.unwrap();
        fs.confirmar(&tx, vec![
            criar(&fs, &format!("games/{jogo}"), campos(&[("published", json!({ "booleanValue": true }))])),
            criar(&fs, &format!("wallets/{uid}"), campos(&[("balance", inteiro(0)), ("version", inteiro(7))])),
        ]).await.unwrap();

        assert!(iniciar(&fs, &uid, &jogo, &Detalhes::default()).await.is_err());
        let leitura = fs.iniciar_transacao().await.unwrap();
        let carteira = fs.ler(&format!("wallets/{uid}"), &leitura).await.unwrap().unwrap();
        assert_eq!(ler_inteiro(&carteira, "balance"), Some(0));
        assert_eq!(ler_inteiro(&carteira, "version"), Some(7));
        fs.desfazer(&leitura).await;
    }
}
