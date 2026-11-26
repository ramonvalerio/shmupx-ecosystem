//! Client Manager do Lab: cliques nos instaladores da página de Download do site (shmupx.com).
//!
//! - `POST /api/client/downloads/click` (público, chamado pelo site com `sendBeacon`): soma o clique
//!   em `downloadStats/{dia}` (`clicks.{plataforma}`, dia no horário de Brasília) e marca o visitante
//!   em `downloadVisitors/{id}` (`days`, `platforms`, `firstAt`, `lastAt`).
//! - O visitante é um id anônimo: SHA-256 do IP com um sal do servidor. O IP nunca é gravado.
//!   Serve para contar pessoas diferentes num período, não para identificar ninguém.
//! - Contra repetição em massa, cada visitante conta no máximo [`CLIQUES_POR_HORA`] por hora.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use chrono::{DateTime, Duration, FixedOffset, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::error::AppError;
use crate::firestore::valor::{data, ler_data, ler_inteiro, ler_lista_de_textos, ler_mapa, ler_texto, texto};
use crate::firestore::{Campos, Firestore};

pub const PLATAFORMAS: [&str; 5] = ["windows", "portable", "appimage", "deb", "android"];
const CLIQUES_POR_HORA: u32 = 20;

#[derive(Debug, Deserialize)]
pub struct Clique {
    pub platform: String,
    #[serde(default)]
    pub version: Option<String>,
}

/// Dia no horário de Brasília (UTC-3, sem horário de verão), como `AAAA-MM-DD`.
pub fn dia_de(quando: DateTime<Utc>) -> String {
    quando.with_timezone(&FixedOffset::west_opt(3 * 3600).expect("fuso fixo")).format("%Y-%m-%d").to_string()
}

/// Id anônimo do visitante: SHA-256 do IP com o sal do servidor.
pub fn visitante(ip: &str) -> String {
    let sal = std::env::var("DOWNLOAD_HASH_SALT")
        .unwrap_or_else(|_| format!("shmupx-download:{}", std::env::var("FIREBASE_PROJECT_ID").unwrap_or_default()));
    let hash = Sha256::digest(format!("{sal}|{}", ip.trim()).as_bytes());
    hash.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// IP de quem clicou: o primeiro do `X-Forwarded-For` (o CloudFront põe o do visitante na frente).
pub fn ip_do_visitante(encaminhado: Option<&str>) -> String {
    encaminhado.and_then(|v| v.split(',').next()).map(str::trim).filter(|v| !v.is_empty()).unwrap_or("desconhecido").to_string()
}

/// Limite por visitante (por instância do backend): `true` se ainda pode contar.
fn dentro_do_limite(id: &str, agora: i64) -> bool {
    static JANELAS: Mutex<Option<HashMap<String, (i64, u32)>>> = Mutex::new(None);
    let Ok(mut guarda) = JANELAS.lock() else { return true };
    let janelas = guarda.get_or_insert_with(HashMap::new);
    if janelas.len() > 50_000 {
        janelas.retain(|_, (inicio, _)| agora - *inicio < 3600);
    }
    let (inicio, n) = janelas.entry(id.to_string()).or_insert((agora, 0));
    if agora - *inicio >= 3600 {
        *inicio = agora;
        *n = 0;
    }
    *n += 1;
    *n <= CLIQUES_POR_HORA
}

/// Registra um clique num instalador. Plataforma desconhecida = `400`.
pub async fn registrar(fs: &Firestore, clique: &Clique, ip: &str) -> Result<bool, AppError> {
    if !PLATAFORMAS.contains(&clique.platform.as_str()) {
        return Err(AppError::bad_request("plataforma inválida"));
    }
    let agora = Utc::now();
    let id = visitante(ip);
    if !dentro_do_limite(&id, agora.timestamp()) {
        return Ok(false);
    }
    let dia = dia_de(agora);
    fs.somar(&[(format!("downloadStats/{dia}"), format!("clicks.{}", clique.platform), 1)]).await?;

    let caminho = format!("downloadVisitors/{id}");
    let novo = fs.ler_documento(&caminho).await?.is_none();
    let mut campos = Campos::new();
    campos.insert("lastAt".into(), data(agora));
    let mut mascara = vec!["lastAt"];
    if novo {
        campos.insert("firstAt".into(), data(agora));
        mascara.push("firstAt");
    }
    if let Some(versao) = clique.version.as_deref().filter(|v| v.len() <= 20 && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')) {
        campos.insert("lastVersion".into(), texto(versao));
        mascara.push("lastVersion");
    }
    fs.gravar(&caminho, campos, &mascara).await?;
    fs.acrescentar_na_lista(&caminho, "days", &[dia]).await?;
    fs.acrescentar_na_lista(&caminho, "platforms", &[clique.platform.clone()]).await?;
    Ok(true)
}

#[derive(Debug, Serialize, PartialEq)]
pub struct DiaDeCliques {
    pub day: String,
    pub clicks: BTreeMap<String, i64>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct VersaoEmUso {
    pub version: String,
    /// Jogadores com esta versão no último acesso.
    pub players: i64,
    /// Desses, os que usaram o Client nos últimos 7 dias.
    pub active_7d: i64,
}

#[derive(Debug, Serialize)]
pub struct PainelClient {
    /// Cliques por dia (horário de Brasília), só os dias com clique.
    pub days: Vec<DiaDeCliques>,
    /// Um item por visitante anônimo: os dias em que clicou (para contar pessoas por período).
    pub visitors: Vec<Vec<String>>,
    pub versions: Vec<VersaoEmUso>,
    /// Instaladores liberados agora (o que a página de Download oferece).
    pub downloads: crate::services::versoes::Downloads,
}

/// Painel do Client Manager: cliques por dia, visitantes e versões em uso.
pub async fn painel(fs: &Firestore) -> Result<PainelClient, AppError> {
    let (dias, visitantes, jogadores, versoes) = tokio::try_join!(
        fs.listar("downloadStats"),
        fs.consultar(json!({ "from": [{ "collectionId": "downloadVisitors" }], "select": { "fields": [{ "fieldPath": "days" }] } })),
        fs.consultar(json!({ "from": [{ "collectionId": "players" }], "select": { "fields": [{ "fieldPath": "clientVersion" }, { "fieldPath": "lastSeenAt" }] } })),
        crate::services::versoes::listar(fs),
    )?;
    let mut days: Vec<DiaDeCliques> = dias
        .into_iter()
        .map(|(day, c)| {
            let mapa = ler_mapa(&c, "clicks").unwrap_or_default();
            let clicks = PLATAFORMAS.iter().filter_map(|p| Some((p.to_string(), ler_inteiro(&mapa, p)?))).collect();
            DiaDeCliques { day, clicks }
        })
        .collect();
    days.sort_by(|a, b| a.day.cmp(&b.day));

    let agora = Utc::now();
    let mut por_versao: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    for (_, c) in &jogadores {
        let Some(versao) = ler_texto(c, "clientVersion") else { continue };
        let ativo = ler_data(c, "lastSeenAt").is_some_and(|q| agora - q < Duration::days(7));
        let entrada = por_versao.entry(versao).or_insert((0, 0));
        entrada.0 += 1;
        entrada.1 += i64::from(ativo);
    }
    let mut versions: Vec<VersaoEmUso> = por_versao.into_iter().map(|(version, (players, active_7d))| VersaoEmUso { version, players, active_7d }).collect();
    versions.sort_by(|a, b| crate::services::versoes::ler_versao(&b.version).cmp(&crate::services::versoes::ler_versao(&a.version)));

    Ok(PainelClient {
        days,
        visitors: visitantes.iter().map(|(_, c)| ler_lista_de_textos(c, "days")).filter(|d| !d.is_empty()).collect(),
        versions,
        downloads: crate::services::versoes::downloads(&versoes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn dia_no_horario_de_brasilia() {
        assert_eq!(dia_de(Utc.with_ymd_and_hms(2026, 11, 25, 2, 59, 0).unwrap()), "2026-11-24");
        assert_eq!(dia_de(Utc.with_ymd_and_hms(2026, 11, 25, 3, 0, 0).unwrap()), "2026-11-25");
    }

    #[test]
    fn visitante_anonimo_e_estavel() {
        let a = visitante("200.1.2.3");
        assert_eq!(a, visitante(" 200.1.2.3 "));
        assert_ne!(a, visitante("200.1.2.4"));
        assert_eq!(a.len(), 32);
        assert!(!a.contains("200"));
    }

    #[test]
    fn ip_vem_do_primeiro_encaminhado() {
        assert_eq!(ip_do_visitante(Some("187.10.0.1, 130.176.0.1, 169.254.1.1")), "187.10.0.1");
        assert_eq!(ip_do_visitante(None), "desconhecido");
    }

    #[test]
    fn limite_por_hora() {
        let id = format!("teste-{}", uuid::Uuid::new_v4());
        assert!((0..CLIQUES_POR_HORA).all(|_| dentro_do_limite(&id, 1000)));
        assert!(!dentro_do_limite(&id, 1000));
        assert!(dentro_do_limite(&id, 1000 + 3600), "janela nova");
    }

    #[tokio::test]
    #[ignore = "exige o emulador do Firestore"]
    async fn conta_cliques_e_visitantes_no_emulador() {
        // SAFETY: teste isolado com --ignored; nenhuma outra thread lê estas variáveis agora.
        unsafe { std::env::set_var("FIREBASE_PROJECT_ID", "demo-shmupx") };
        let fs = Firestore::do_ambiente().unwrap();
        let ip = format!("10.{}.0.1", uuid::Uuid::new_v4().as_u128() % 250);
        let antes = painel(&fs).await.unwrap();
        let hoje = dia_de(Utc::now());
        let cliques_hoje = |p: &PainelClient, plataforma: &str| p.days.iter().find(|d| d.day == hoje).and_then(|d| d.clicks.get(plataforma).copied()).unwrap_or(0);

        assert!(registrar(&fs, &Clique { platform: "windows".into(), version: Some("0.1.16".into()) }, &ip).await.unwrap());
        assert!(registrar(&fs, &Clique { platform: "windows".into(), version: None }, &ip).await.unwrap());
        assert!(registrar(&fs, &Clique { platform: "deb".into(), version: None }, &ip).await.unwrap());
        assert!(registrar(&fs, &Clique { platform: "mac".into(), version: None }, &ip).await.is_err());

        let depois = painel(&fs).await.unwrap();
        assert_eq!(cliques_hoje(&depois, "windows") - cliques_hoje(&antes, "windows"), 2);
        assert_eq!(cliques_hoje(&depois, "deb") - cliques_hoje(&antes, "deb"), 1);
        assert_eq!(depois.visitors.len(), antes.visitors.len() + 1, "o mesmo IP é uma pessoa só");
        let doc = fs.ler_documento(&format!("downloadVisitors/{}", visitante(&ip))).await.unwrap().unwrap();
        assert_eq!(ler_lista_de_textos(&doc, "platforms"), vec!["windows".to_string(), "deb".to_string()]);
        assert!(!serde_json::to_string(&doc).unwrap().contains(&ip), "o IP não é gravado");
    }
}
