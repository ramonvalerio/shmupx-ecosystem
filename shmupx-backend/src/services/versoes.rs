//! Versões do ShmupX Client: `releases/{versão}` no Firestore.
//!
//! O admin escreve a versão no Lab (rascunho), com as novidades, correções, mudanças de regra e
//! os scripts de validação que ela leva. Ao **agendar**, escolhe a antecedência do aviso (urgente,
//! 24 h, 48 h ou 7 dias): o servidor fixa `releaseAt`, publica a notificação para os jogadores e
//! dispara os e-mails. O status depois disso é calculado pelo relógio do servidor: `scheduled`
//! até `releaseAt`, `released` a partir dele. Uma versão marcada como obrigatória bloqueia novas
//! runs de Clients mais antigos assim que é liberada (`exige_atualizacao`).

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::firestore::valor::{booleano, data, inteiro, ler_booleano, ler_data, ler_inteiro, ler_lista_de_textos, ler_texto, lista_de_textos, texto};
use crate::firestore::{Campos, Firestore};
use crate::services::gameplays::id_valido;

const TITULO_MAXIMO: usize = 80;
const ITEM_MAXIMO: usize = 300;
const ITENS_MAXIMOS: usize = 40;

/// Antecedência entre o aviso (agendamento) e a liberação no Client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Antecedencia {
    /// Libera na hora (correção urgente), como era antes dos avisos.
    Urgente,
    #[serde(rename = "24h")]
    Horas24,
    #[serde(rename = "48h")]
    Horas48,
    #[serde(rename = "7d")]
    Dias7,
}

impl Antecedencia {
    fn duracao(self) -> Duration {
        match self {
            Antecedencia::Urgente => Duration::zero(),
            Antecedencia::Horas24 => Duration::hours(24),
            Antecedencia::Horas48 => Duration::hours(48),
            Antecedencia::Dias7 => Duration::days(7),
        }
    }

    fn texto(self) -> &'static str {
        match self {
            Antecedencia::Urgente => "urgent",
            Antecedencia::Horas24 => "24h",
            Antecedencia::Horas48 => "48h",
            Antecedencia::Dias7 => "7d",
        }
    }

    fn de_texto(valor: &str) -> Option<Self> {
        [Antecedencia::Urgente, Antecedencia::Horas24, Antecedencia::Horas48, Antecedencia::Dias7]
            .into_iter()
            .find(|a| a.texto() == valor)
    }
}

/// O que o Lab grava num rascunho. A versão vem da rota, nunca do corpo.
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VersaoEntrada {
    pub title: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub fixes: Vec<String>,
    #[serde(default)]
    pub rules: Vec<String>,
    /// Scripts de validação incluídos, como `jogo@versão` (ex.: `aero-fighters-2@0.2.0`).
    #[serde(default)]
    pub scripts: Vec<String>,
    #[serde(default)]
    pub mandatory: bool,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Versao {
    pub version: String,
    pub title: String,
    pub features: Vec<String>,
    pub fixes: Vec<String>,
    pub rules: Vec<String>,
    pub scripts: Vec<String>,
    pub mandatory: bool,
    /// `draft`, `scheduled`, `released` ou `cancelled` (os dois do meio pelo relógio do servidor).
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lead: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_at: Option<DateTime<Utc>>,
    /// Resumo do envio de e-mails (`not_configured`, `sending`, `sent N`, `failed: ...`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emails_sent: Option<i64>,
}

/// `1.2.3` → (1, 2, 3). Três números sem zeros à esquerda.
pub fn ler_versao(texto: &str) -> Option<(u32, u32, u32)> {
    let mut partes = texto.split('.');
    let mut parte = || -> Option<u32> {
        let p = partes.next()?;
        let ok = !p.is_empty() && p.len() <= 4 && p.chars().all(|c| c.is_ascii_digit()) && (p == "0" || !p.starts_with('0'));
        if ok { p.parse().ok() } else { None }
    };
    let versao = (parte()?, parte()?, parte()?);
    partes.next().is_none().then_some(versao)
}

fn lista(itens: Vec<String>, campo: &str) -> Result<Vec<String>, AppError> {
    let itens: Vec<String> = itens.into_iter().map(|i| i.trim().to_string()).filter(|i| !i.is_empty()).collect();
    if itens.len() > ITENS_MAXIMOS {
        return Err(AppError::bad_request(format!("{campo}: no máximo {ITENS_MAXIMOS} itens")));
    }
    if itens.iter().any(|i| i.chars().count() > ITEM_MAXIMO) {
        return Err(AppError::bad_request(format!("{campo}: cada item com no máximo {ITEM_MAXIMO} caracteres")));
    }
    Ok(itens)
}

fn validar(versao: &str, entrada: VersaoEntrada) -> Result<VersaoEntrada, AppError> {
    if ler_versao(versao).is_none() {
        return Err(AppError::bad_request("versão inválida (use o formato 1.2.3)"));
    }
    let title = entrada.title.trim().to_string();
    if title.is_empty() || title.chars().count() > TITULO_MAXIMO {
        return Err(AppError::bad_request("título deve ter de 1 a 80 caracteres"));
    }
    let scripts = lista(entrada.scripts, "scripts")?;
    for script in &scripts {
        let valido = script.split_once('@').is_some_and(|(jogo, v)| id_valido(jogo) && ler_versao(v).is_some());
        if !valido {
            return Err(AppError::bad_request(format!("script inválido: {script} (use jogo@1.2.3)")));
        }
    }
    let features = lista(entrada.features, "features")?;
    let fixes = lista(entrada.fixes, "fixes")?;
    let rules = lista(entrada.rules, "rules")?;
    if features.is_empty() && fixes.is_empty() && rules.is_empty() && scripts.is_empty() {
        return Err(AppError::bad_request("a versão precisa de ao menos uma novidade, correção, regra ou script"));
    }
    Ok(VersaoEntrada { title, features, fixes, rules, scripts, mandatory: entrada.mandatory })
}

/// Status efetivo: um agendamento vira `released` quando chega a hora.
fn status_efetivo(guardado: &str, release_at: Option<DateTime<Utc>>, agora: DateTime<Utc>) -> String {
    match (guardado, release_at) {
        ("scheduled", Some(quando)) if agora >= quando => "released".into(),
        (outro, _) => outro.into(),
    }
}

fn versao_de(version: String, campos: &Campos, agora: DateTime<Utc>) -> Versao {
    let release_at = ler_data(campos, "releaseAt");
    Versao {
        title: ler_texto(campos, "title").unwrap_or_else(|| version.clone()),
        features: ler_lista_de_textos(campos, "features"),
        fixes: ler_lista_de_textos(campos, "fixes"),
        rules: ler_lista_de_textos(campos, "rules"),
        scripts: ler_lista_de_textos(campos, "scripts"),
        mandatory: ler_booleano(campos, "mandatory").unwrap_or(false),
        status: status_efetivo(&ler_texto(campos, "status").unwrap_or_else(|| "draft".into()), release_at, agora),
        lead: ler_texto(campos, "lead"),
        scheduled_at: ler_data(campos, "scheduledAt"),
        release_at,
        email_status: ler_texto(campos, "emailStatus"),
        emails_sent: ler_inteiro(campos, "emailsSent"),
        version,
    }
}

/// Todas as versões, da mais nova para a mais antiga.
pub async fn listar(fs: &Firestore) -> Result<Vec<Versao>, AppError> {
    let agora = Utc::now();
    let mut versoes: Vec<Versao> =
        fs.listar("releases").await?.into_iter().map(|(id, campos)| versao_de(id, &campos, agora)).collect();
    versoes.sort_by_key(|v| std::cmp::Reverse(ler_versao(&v.version)));
    Ok(versoes)
}

pub async fn obter(fs: &Firestore, versao: &str) -> Result<Versao, AppError> {
    if ler_versao(versao).is_none() {
        return Err(AppError::bad_request("versão inválida"));
    }
    let campos = fs.ler_documento(&format!("releases/{versao}")).await?.ok_or_else(|| AppError::not_found("versão não encontrada"))?;
    Ok(versao_de(versao.to_string(), &campos, Utc::now()))
}

const MASCARA_RASCUNHO: [&str; 9] = ["title", "features", "fixes", "rules", "scripts", "mandatory", "status", "updatedAt", "version"];

/// Cria ou atualiza o rascunho. Versões já agendadas não mudam (o aviso já saiu com aquele texto).
pub async fn salvar_rascunho(fs: &Firestore, versao: &str, entrada: VersaoEntrada) -> Result<Versao, AppError> {
    let entrada = validar(versao, entrada)?;
    if let Some(campos) = fs.ler_documento(&format!("releases/{versao}")).await? {
        let atual = ler_texto(&campos, "status").unwrap_or_else(|| "draft".into());
        if atual != "draft" {
            return Err(AppError::conflict("só rascunhos podem ser editados; crie uma nova versão"));
        }
    }
    let mut campos = Campos::new();
    campos.insert("version".into(), texto(versao));
    campos.insert("title".into(), texto(&entrada.title));
    campos.insert("features".into(), lista_de_textos(&entrada.features));
    campos.insert("fixes".into(), lista_de_textos(&entrada.fixes));
    campos.insert("rules".into(), lista_de_textos(&entrada.rules));
    campos.insert("scripts".into(), lista_de_textos(&entrada.scripts));
    campos.insert("mandatory".into(), booleano(entrada.mandatory));
    campos.insert("status".into(), texto("draft"));
    campos.insert("updatedAt".into(), data(Utc::now()));
    fs.gravar(&format!("releases/{versao}"), campos, &MASCARA_RASCUNHO).await?;
    obter(fs, versao).await
}

/// Agenda o rascunho: fixa `releaseAt = agora + antecedência`. A versão precisa ser maior que
/// todas as já agendadas ou liberadas (o Client só anda para a frente).
pub async fn agendar(fs: &Firestore, versao: &str, antecedencia: Antecedencia) -> Result<Versao, AppError> {
    let atual = obter(fs, versao).await?;
    if atual.status != "draft" {
        return Err(AppError::conflict("só rascunhos podem ser agendados"));
    }
    let nova = ler_versao(versao);
    let maior_ativa = listar(fs)
        .await?
        .into_iter()
        .filter(|v| v.status == "scheduled" || v.status == "released")
        .filter_map(|v| ler_versao(&v.version))
        .max();
    if maior_ativa.is_some_and(|maior| nova <= Some(maior)) {
        return Err(AppError::conflict("a versão precisa ser maior que todas as já agendadas ou liberadas"));
    }
    let agora = Utc::now();
    let mut campos = Campos::new();
    campos.insert("status".into(), texto("scheduled"));
    campos.insert("lead".into(), texto(antecedencia.texto()));
    campos.insert("scheduledAt".into(), data(agora));
    campos.insert("releaseAt".into(), data(agora + antecedencia.duracao()));
    fs.gravar(&format!("releases/{versao}"), campos, &["status", "lead", "scheduledAt", "releaseAt"]).await?;
    obter(fs, versao).await
}

/// Cancela um agendamento que ainda não foi liberado.
pub async fn cancelar(fs: &Firestore, versao: &str) -> Result<Versao, AppError> {
    let atual = obter(fs, versao).await?;
    if atual.status != "scheduled" {
        return Err(AppError::conflict("só versões agendadas (ainda não liberadas) podem ser canceladas"));
    }
    let mut campos = Campos::new();
    campos.insert("status".into(), texto("cancelled"));
    campos.insert("updatedAt".into(), data(Utc::now()));
    fs.gravar(&format!("releases/{versao}"), campos, &["status", "updatedAt"]).await?;
    obter(fs, versao).await
}

/// Grava o resultado do envio de e-mails na versão (para o Lab mostrar).
pub async fn registrar_emails(fs: &Firestore, versao: &str, status: &str, enviados: i64) -> Result<(), AppError> {
    let mut campos = Campos::new();
    campos.insert("emailStatus".into(), texto(status));
    campos.insert("emailsSent".into(), inteiro(enviados));
    fs.gravar(&format!("releases/{versao}"), campos, &["emailStatus", "emailsSent"]).await
}

/// Situação do Client que chama: a versão mais nova liberada e se ele precisa atualizar antes de
/// abrir uma run (existe versão obrigatória liberada maior que a dele).
#[derive(Debug, Serialize, PartialEq)]
pub struct SituacaoClient {
    pub latest: Option<String>,
    pub update_available: bool,
    pub update_required: bool,
    /// Próxima versão agendada (aviso antecipado), se houver.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_release_at: Option<DateTime<Utc>>,
}

pub fn situacao(versoes: &[Versao], atual: Option<&str>) -> SituacaoClient {
    // Client sem versão informada (anterior aos avisos) conta como 0.0.0.
    let atual = atual.and_then(ler_versao).unwrap_or((0, 0, 0));
    let liberadas: Vec<&Versao> = versoes.iter().filter(|v| v.status == "released").collect();
    let latest = liberadas.iter().filter_map(|v| ler_versao(&v.version).map(|n| (n, *v))).max_by_key(|(n, _)| *n);
    let update_required = liberadas.iter().any(|v| v.mandatory && ler_versao(&v.version).is_some_and(|n| n > atual));
    let proxima = versoes
        .iter()
        .filter(|v| v.status == "scheduled")
        .filter_map(|v| ler_versao(&v.version).map(|n| (n, v)))
        .min_by_key(|(n, _)| *n);
    SituacaoClient {
        update_available: latest.is_some_and(|(n, _)| n > atual),
        latest: latest.map(|(_, v)| v.version.clone()),
        update_required,
        next: proxima.map(|(_, v)| v.version.clone()),
        next_release_at: proxima.and_then(|(_, v)| v.release_at),
    }
}

/// Recusa a run se o Client está abaixo de uma versão obrigatória já liberada.
pub async fn exigir_versao_em_dia(fs: &Firestore, atual: Option<&str>) -> Result<(), AppError> {
    let situacao = situacao(&listar(fs).await?, atual);
    if situacao.update_required {
        return Err(AppError::conflict(format!(
            "atualize o ShmupX Client para a versão {} antes de jogar",
            situacao.latest.unwrap_or_default()
        )));
    }
    Ok(())
}

pub fn antecedencia_de(valor: &str) -> Option<Antecedencia> {
    Antecedencia::de_texto(valor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entrada() -> VersaoEntrada {
        VersaoEntrada { title: " Aero Fighters 2 ".into(), features: vec!["Sala do Aero Fighters 2".into()], ..Default::default() }
    }

    fn versao(v: &str, status: &str, mandatory: bool) -> Versao {
        Versao { version: v.into(), status: status.into(), mandatory, ..Default::default() }
    }

    #[test]
    fn entrada_valida_e_normalizada() {
        let ok = validar("0.2.0", VersaoEntrada { fixes: vec!["  ".into(), " Gravação ".into()], ..entrada() }).unwrap();
        assert_eq!(ok.title, "Aero Fighters 2");
        assert_eq!(ok.fixes, vec!["Gravação"]);
    }

    #[test]
    fn entradas_invalidas_sao_recusadas() {
        assert!(validar("0.2", entrada()).is_err());
        assert!(validar("../0.2.0", entrada()).is_err());
        assert!(validar("0.2.0", VersaoEntrada { title: " ".into(), ..entrada() }).is_err());
        assert!(validar("0.2.0", VersaoEntrada { features: vec![], ..entrada() }).is_err(), "versão vazia");
        assert!(validar("0.2.0", VersaoEntrada { scripts: vec!["aero-fighters-2".into()], ..entrada() }).is_err());
        assert!(validar("0.2.0", VersaoEntrada { scripts: vec!["../x@0.1.0".into()], ..entrada() }).is_err());
        assert!(validar("0.2.0", VersaoEntrada { features: vec!["x".repeat(301)], ..entrada() }).is_err());
        assert!(validar("0.2.0", VersaoEntrada { scripts: vec!["aero-fighters-2@0.2.0".into()], ..entrada() }).is_ok());
    }

    #[test]
    fn corpo_com_campo_desconhecido_e_recusado() {
        assert!(serde_json::from_str::<VersaoEntrada>(r#"{"title":"x","status":"released"}"#).is_err());
    }

    #[test]
    fn agendada_vira_liberada_na_hora_marcada() {
        let agora = Utc::now();
        assert_eq!(status_efetivo("scheduled", Some(agora + Duration::hours(1)), agora), "scheduled");
        assert_eq!(status_efetivo("scheduled", Some(agora), agora), "released");
        assert_eq!(status_efetivo("cancelled", Some(agora - Duration::hours(1)), agora), "cancelled");
        assert_eq!(status_efetivo("draft", None, agora), "draft");
    }

    #[test]
    fn antecedencias_aceitas() {
        assert_eq!(antecedencia_de("urgent"), Some(Antecedencia::Urgente));
        assert_eq!(antecedencia_de("48h").map(|a| a.duracao()), Some(Duration::hours(48)));
        assert_eq!(antecedencia_de("1h"), None);
    }

    #[test]
    fn situacao_do_client() {
        let versoes = vec![
            versao("0.2.0", "released", false),
            versao("0.3.0", "released", true),
            versao("0.4.0", "scheduled", true),
            versao("0.5.0", "draft", true),
            versao("0.9.0", "cancelled", true),
        ];
        let antigo = situacao(&versoes, Some("0.1.0"));
        assert_eq!(antigo.latest.as_deref(), Some("0.3.0"));
        assert!(antigo.update_available && antigo.update_required);
        assert_eq!(antigo.next.as_deref(), Some("0.4.0"));

        let em_dia = situacao(&versoes, Some("0.3.0"));
        assert!(!em_dia.update_available && !em_dia.update_required, "agendada ainda não obriga");

        assert!(situacao(&versoes, None).update_required, "Client sem versão conta como 0.0.0");
        assert!(!situacao(&[versao("0.2.0", "released", false)], Some("0.1.0")).update_required);
    }
}
