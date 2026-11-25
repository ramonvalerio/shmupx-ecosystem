//! Users Manager do Lab: todos os jogadores cadastrados, quem está online, ausente ou offline, e a
//! moderação (banir, mutar, desativar o acesso, tirar a visibilidade no chat).
//!
//! - `moderation/{uid}`: `banned`, `muted`, `disabled`, `chatHidden`, `note`, `updatedAt`,
//!   `updatedBy`. Só o servidor grava; o próprio jogador lê o seu (o Client mostra o aviso e trava
//!   o chat). As Security Rules leem o documento para recusar mensagens e presença.
//! - `chatModeration/hidden`: `uids` de quem não aparece no chat (sem visibilidade ou banido). Todo
//!   jogador logado lê: o Client esconde as mensagens dessas pessoas. Quem está sem visibilidade
//!   também não lê nem posta no chat (Rules).
//! - Situação: a presença nas salas (`rooms/*/presence/{uid}`, a cada 30 s) e o sinal do Client
//!   aberto (`remoteLaunch/{uid}.desktop.seenAt`, a cada 45 s).

use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::AppError;
use crate::firestore::valor::{booleano, data, ler_booleano, ler_data, ler_inteiro, ler_mapa, ler_texto, lista_de_textos, texto};
use crate::firestore::{Campos, Firestore};
use crate::middleware::firebase::FirebaseUser;
use crate::services::gameplays::id_valido;

/// Presença mais velha que isso já é offline (o mesmo `STALE_MS` do Client).
const PRESENCA_VALIDA_SEGUNDOS: i64 = 75;
/// Client aberto: o sinal vem a cada 45 s.
const CLIENT_ABERTO_SEGUNDOS: i64 = 120;
/// Jogo aberto na conta: o Client renova a cada minuto (o mesmo `PC_STALE_MS`).
const JOGANDO_SEGUNDOS: i64 = 180;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Moderacao {
    pub banned: bool,
    pub muted: bool,
    pub disabled: bool,
    pub chat_hidden: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UsuarioAdmin {
    pub uid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
    /// Última alteração do perfil (nome, idioma).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
    /// Tempo total com o ShmupX aberto (online, ausente, jogando ou ao vivo), contado minuto a minuto.
    pub connected_seconds: i64,
    /// Último sinal de vida (backend, sala ou Client aberto).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_version: Option<String>,
    /// `None`: ainda sem carteira (nasce com as fichas iniciais na primeira run).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<i64>,
    /// `live`, `playing`, `online`, `away` ou `offline`.
    pub status: String,
    /// O que está fazendo: jogo aberto ou sala em que está.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<String>,
    pub is_admin: bool,
    pub moderation: Moderacao,
    /// Denúncias do chat ainda abertas contra o jogador.
    pub reports_open: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModeracaoEntrada {
    #[serde(default)]
    pub banned: Option<bool>,
    #[serde(default)]
    pub muted: Option<bool>,
    #[serde(default)]
    pub disabled: Option<bool>,
    #[serde(default)]
    pub chat_hidden: Option<bool>,
    /// Motivo da última mudança (fica no histórico do documento).
    #[serde(default)]
    pub note: Option<String>,
}

fn moderacao_de(c: &Campos) -> Moderacao {
    Moderacao {
        banned: ler_booleano(c, "banned").unwrap_or(false),
        muted: ler_booleano(c, "muted").unwrap_or(false),
        disabled: ler_booleano(c, "disabled").unwrap_or(false),
        chat_hidden: ler_booleano(c, "chatHidden").unwrap_or(false),
        note: ler_texto(c, "note"),
        updated_at: ler_data(c, "updatedAt"),
        updated_by: ler_texto(c, "updatedBy"),
    }
}

fn selecionar(colecao: &str, campos: &[&str]) -> serde_json::Value {
    json!({
        "from": [{ "collectionId": colecao }],
        "select": { "fields": campos.iter().map(|c| json!({ "fieldPath": c })).collect::<Vec<_>>() },
    })
}

fn de_milissegundos(ms: i64) -> Option<DateTime<Utc>> {
    Utc.timestamp_millis_opt(ms).single()
}

/// Presença mais "forte" vence: ao vivo > jogando > online > ausente.
fn peso(estado: &str) -> u8 {
    match estado {
        "live" => 4,
        "playing" => 3,
        "online" => 2,
        "away" => 1,
        _ => 0,
    }
}

/// Sinais de vida de um jogador, para decidir a situação.
#[derive(Debug, Default, Clone, PartialEq)]
struct Sinais {
    /// Melhor presença ainda válida numa sala: (estado, título da sala ou do jogo).
    presenca: Option<(String, Option<String>)>,
    client_aberto: bool,
    /// Jogo aberto na conta agora.
    jogando: Option<String>,
}

fn situacao(sinais: &Sinais) -> (String, Option<String>) {
    let presenca = sinais.presenca.clone();
    if let Some((estado, titulo)) = &presenca
        && estado == "live"
    {
        return ("live".into(), titulo.clone());
    }
    if let Some(jogo) = &sinais.jogando {
        return ("playing".into(), Some(jogo.clone()));
    }
    match presenca {
        Some((estado, titulo)) => (estado, titulo),
        None if sinais.client_aberto => ("online".into(), None),
        None => ("offline".into(), None),
    }
}

fn consulta_presencas() -> serde_json::Value {
    json!({
        "from": [{ "collectionId": "presence", "allDescendants": true }],
        "select": { "fields": [{ "fieldPath": "state" }, { "fieldPath": "lastSeen" }, { "fieldPath": "playingTitle" }] },
    })
}

fn consulta_remotos() -> serde_json::Value {
    selecionar("remoteLaunch", &["desktop.seenAt", "playing.title", "playing.beatAt"])
}

/// Sinais de cada jogador agora (`rooms/*/presence/{uid}` e `remoteLaunch/{uid}`) e o último
/// momento em que cada um deu sinal de vida.
fn montar_sinais(
    presencas: &[(String, Campos)],
    remotos: &[(String, Campos)],
    titulos: &HashMap<String, String>,
    agora: DateTime<Utc>,
) -> (HashMap<String, Sinais>, HashMap<String, DateTime<Utc>>) {
    let mut sinais: HashMap<String, Sinais> = HashMap::new();
    let mut vistos: HashMap<String, DateTime<Utc>> = HashMap::new();
    let mut ver = |uid: &str, quando: Option<DateTime<Utc>>| {
        if let Some(q) = quando {
            let v = vistos.entry(uid.to_string()).or_insert(q);
            *v = (*v).max(q);
        }
    };
    for (caminho, c) in presencas {
        let partes: Vec<&str> = caminho.split('/').collect();
        let [_, sala, _, uid] = partes.as_slice() else { continue };
        let visto = ler_data(c, "lastSeen");
        ver(uid, visto);
        if visto.is_none_or(|v| agora - v >= Duration::seconds(PRESENCA_VALIDA_SEGUNDOS)) {
            continue;
        }
        let estado = ler_texto(c, "state").unwrap_or_else(|| "online".into());
        let titulo = ler_texto(c, "playingTitle").filter(|_| estado == "playing").or_else(|| titulos.get(*sala).cloned());
        let s = sinais.entry(uid.to_string()).or_default();
        if s.presenca.as_ref().is_none_or(|(atual, _)| peso(&estado) > peso(atual)) {
            s.presenca = Some((estado, titulo));
        }
    }
    for (uid, c) in remotos {
        let desktop = ler_mapa(c, "desktop").and_then(|d| ler_inteiro(&d, "seenAt")).and_then(de_milissegundos);
        ver(uid, desktop);
        let s = sinais.entry(uid.clone()).or_default();
        s.client_aberto = desktop.is_some_and(|d| agora - d < Duration::seconds(CLIENT_ABERTO_SEGUNDOS));
        if let Some(jogo) = ler_mapa(c, "playing") {
            let batida = ler_inteiro(&jogo, "beatAt").and_then(de_milissegundos);
            if batida.is_some_and(|b| agora - b < Duration::seconds(JOGANDO_SEGUNDOS)) {
                s.jogando = ler_texto(&jogo, "title");
            }
        }
    }
    (sinais, vistos)
}

/// Quem está conectado agora (online, ausente, jogando ou ao vivo): conta para o tempo conectado.
pub async fn conectados_agora(fs: &Firestore) -> Result<Vec<String>, AppError> {
    let (presencas, remotos) = tokio::try_join!(fs.consultar_caminhos(consulta_presencas()), fs.consultar(consulta_remotos()))?;
    let (sinais, _) = montar_sinais(&presencas, &remotos, &HashMap::new(), Utc::now());
    Ok(sinais.into_iter().filter(|(uid, s)| id_valido(uid) && situacao(s).0 != "offline").map(|(uid, _)| uid).collect())
}

/// Todos os jogadores com a situação de agora e a moderação.
pub async fn listar(fs: &Firestore) -> Result<Vec<UsuarioAdmin>, AppError> {
    let agora = Utc::now();
    let (usuarios, jogadores, perfis, carteiras, moderacoes, presencas, remotos, jogos, denuncias) = tokio::try_join!(
        fs.consultar(selecionar("users", &["displayName", "createdAt", "updatedAt"])),
        fs.consultar(selecionar("players", &["email", "lastSeenAt", "clientVersion", "connectedSeconds"])),
        fs.consultar(selecionar("publicProfiles", &["handle", "displayName"])),
        fs.consultar(selecionar("wallets", &["balance"])),
        fs.listar("moderation"),
        fs.consultar_caminhos(consulta_presencas()),
        fs.consultar(consulta_remotos()),
        fs.consultar(selecionar("games", &["title"])),
        crate::services::denuncias::todas(fs),
    )?;
    let denunciados = crate::services::denuncias::abertas_por_autor(&denuncias);

    let titulos: HashMap<String, String> = jogos.into_iter().filter_map(|(id, c)| Some((id, ler_texto(&c, "title")?))).collect();
    let usuarios: HashMap<String, Campos> = usuarios.into_iter().collect();
    let jogadores: HashMap<String, Campos> = jogadores.into_iter().collect();
    let perfis: HashMap<String, Campos> = perfis.into_iter().collect();
    let carteiras: HashMap<String, Campos> = carteiras.into_iter().collect();
    let moderacoes: HashMap<String, Campos> = moderacoes.into_iter().collect();

    let (sinais, vistos) = montar_sinais(&presencas, &remotos, &titulos, agora);

    let uids: BTreeSet<&String> = usuarios.keys().chain(jogadores.keys()).filter(|uid| id_valido(uid)).collect();
    let mut lista: Vec<UsuarioAdmin> = uids
        .into_iter()
        .map(|uid| {
            let usuario = usuarios.get(uid);
            let jogador = jogadores.get(uid);
            let perfil = perfis.get(uid);
            let email = jogador.and_then(|c| ler_texto(c, "email"));
            let ultimo = [jogador.and_then(|c| ler_data(c, "lastSeenAt")), vistos.get(uid).copied()].into_iter().flatten().max();
            let (status, activity) = situacao(sinais.get(uid).unwrap_or(&Sinais::default()));
            UsuarioAdmin {
                uid: uid.clone(),
                display_name: usuario.and_then(|c| ler_texto(c, "displayName")).or_else(|| perfil.and_then(|c| ler_texto(c, "displayName"))),
                handle: perfil.and_then(|c| ler_texto(c, "handle")),
                is_admin: email.as_ref().is_some_and(|e| FirebaseUser { uid: uid.clone(), email: Some(e.clone()), email_verified: true }.e_admin()),
                email,
                created_at: usuario.and_then(|c| ler_data(c, "createdAt")),
                updated_at: usuario.and_then(|c| ler_data(c, "updatedAt")),
                connected_seconds: jogador.and_then(|c| ler_inteiro(c, "connectedSeconds")).unwrap_or(0),
                last_seen_at: ultimo,
                client_version: jogador.and_then(|c| ler_texto(c, "clientVersion")),
                balance: carteiras.get(uid).and_then(|c| ler_inteiro(c, "balance")),
                status,
                activity,
                moderation: moderacoes.get(uid).map(moderacao_de).unwrap_or_default(),
                reports_open: denunciados.get(uid.as_str()).copied().unwrap_or(0),
            }
        })
        .collect();
    // Quem está por aí primeiro; depois quem passou mais recentemente.
    lista.sort_by(|a, b| (b.status != "offline").cmp(&(a.status != "offline")).then(b.last_seen_at.cmp(&a.last_seen_at)));
    Ok(lista)
}

/// Aplica a moderação (só os campos enviados mudam) e atualiza quem some do chat.
pub async fn moderar(fs: &Firestore, uid: &str, entrada: ModeracaoEntrada, admin: &FirebaseUser) -> Result<Moderacao, AppError> {
    if !id_valido(uid) {
        return Err(AppError::bad_request("jogador inválido"));
    }
    if uid == admin.uid {
        return Err(AppError::bad_request("você não pode moderar a própria conta"));
    }
    let lidos = fs.ler_varios(&[format!("users/{uid}"), format!("players/{uid}"), format!("moderation/{uid}")]).await?;
    if !lidos.contains_key(&format!("users/{uid}")) && !lidos.contains_key(&format!("players/{uid}")) {
        return Err(AppError::not_found("jogador não encontrado"));
    }
    if let Some(email) = lidos.get(&format!("players/{uid}")).and_then(|c| ler_texto(c, "email"))
        && (FirebaseUser { uid: uid.to_string(), email: Some(email), email_verified: true }).e_admin()
    {
        return Err(AppError::bad_request("não é possível moderar um administrador do Lab"));
    }
    let nota = entrada.note.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(str::to_string);
    if nota.as_ref().is_some_and(|n| n.chars().count() > 300) {
        return Err(AppError::bad_request("motivo longo demais (até 300 caracteres)"));
    }

    let atual = lidos.get(&format!("moderation/{uid}")).map(moderacao_de).unwrap_or_default();
    let nova = Moderacao {
        banned: entrada.banned.unwrap_or(atual.banned),
        muted: entrada.muted.unwrap_or(atual.muted),
        disabled: entrada.disabled.unwrap_or(atual.disabled),
        chat_hidden: entrada.chat_hidden.unwrap_or(atual.chat_hidden),
        note: nota.or(atual.note),
        updated_at: Some(Utc::now()),
        updated_by: admin.email.clone().or_else(|| Some(admin.uid.clone())),
    };
    let mut campos = Campos::new();
    campos.insert("banned".into(), booleano(nova.banned));
    campos.insert("muted".into(), booleano(nova.muted));
    campos.insert("disabled".into(), booleano(nova.disabled));
    campos.insert("chatHidden".into(), booleano(nova.chat_hidden));
    if let Some(nota) = &nova.note {
        campos.insert("note".into(), texto(nota));
    }
    campos.insert("updatedAt".into(), data(nova.updated_at.unwrap_or_else(Utc::now)));
    campos.insert("updatedBy".into(), texto(nova.updated_by.as_deref().unwrap_or_default()));
    fs.gravar(&format!("moderation/{uid}"), campos, &["banned", "muted", "disabled", "chatHidden", "note", "updatedAt", "updatedBy"]).await?;
    atualizar_ocultos(fs).await?;
    println!(
        "usuários: {} moderou {uid} (banido={}, mudo={}, desativado={}, sem chat={})",
        nova.updated_by.as_deref().unwrap_or_default(),
        nova.banned,
        nova.muted,
        nova.disabled,
        nova.chat_hidden
    );
    Ok(nova)
}

/// Reescreve `chatModeration/hidden` a partir das moderações (sem visibilidade no chat ou banidos).
async fn atualizar_ocultos(fs: &Firestore) -> Result<(), AppError> {
    let mut uids: Vec<String> = fs
        .listar("moderation")
        .await?
        .into_iter()
        .filter(|(_, c)| ler_booleano(c, "chatHidden") == Some(true) || ler_booleano(c, "banned") == Some(true))
        .map(|(uid, _)| uid)
        .collect();
    uids.sort();
    let mut campos = Campos::new();
    campos.insert("uids".into(), lista_de_textos(&uids));
    campos.insert("updatedAt".into(), data(Utc::now()));
    fs.gravar("chatModeration/hidden", campos, &["uids", "updatedAt"]).await
}

/// Conta banida ou desativada não joga valendo ranking. `None` = liberada.
pub async fn bloqueio(fs: &Firestore, uid: &str) -> Result<Option<&'static str>, AppError> {
    let Some(c) = fs.ler_documento(&format!("moderation/{uid}")).await? else { return Ok(None) };
    let m = moderacao_de(&c);
    Ok(if m.banned {
        Some("esta conta foi banida do ShmupX")
    } else if m.disabled {
        Some("o acesso desta conta está desativado")
    } else {
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn situacao_pelos_sinais() {
        let nada = Sinais::default();
        assert_eq!(situacao(&nada), ("offline".into(), None));
        let aberto = Sinais { client_aberto: true, ..Default::default() };
        assert_eq!(situacao(&aberto), ("online".into(), None));
        let ausente = Sinais { presenca: Some(("away".into(), Some("Darius".into()))), client_aberto: true, ..Default::default() };
        assert_eq!(situacao(&ausente), ("away".into(), Some("Darius".into())));
        let jogando = Sinais { presenca: Some(("away".into(), None)), jogando: Some("Earthion".into()), ..Default::default() };
        assert_eq!(situacao(&jogando), ("playing".into(), Some("Earthion".into())));
        let live = Sinais { presenca: Some(("live".into(), Some("Darius".into()))), jogando: Some("Darius".into()), ..Default::default() };
        assert_eq!(situacao(&live), ("live".into(), Some("Darius".into())));
    }

    #[test]
    fn presenca_mais_forte_vence() {
        assert!(peso("live") > peso("playing"));
        assert!(peso("playing") > peso("online"));
        assert!(peso("online") > peso("away"));
        assert!(peso("away") > peso("qualquer"));
    }

    #[tokio::test]
    #[ignore = "exige o emulador do Firestore"]
    async fn moderacao_e_situacao_no_emulador() {
        // SAFETY: teste isolado com --ignored; nenhuma outra thread lê estas variáveis agora.
        unsafe { std::env::set_var("FIREBASE_PROJECT_ID", "demo-shmupx") };
        let fs = Firestore::do_ambiente().unwrap();
        let uid = format!("u-{}", uuid::Uuid::new_v4());
        let agora = Utc::now();
        let mut usuario = Campos::new();
        usuario.insert("displayName".into(), texto("Piloto"));
        usuario.insert("createdAt".into(), data(agora));
        fs.gravar_novo(&format!("users/{uid}"), usuario).await.unwrap();
        let mut jogo = Campos::new();
        jogo.insert("title".into(), texto("Darius Gaiden"));
        fs.gravar("games/darius-teste", jogo, &["title"]).await.unwrap();
        let mut presenca = Campos::new();
        presenca.insert("state".into(), texto("away"));
        presenca.insert("lastSeen".into(), data(agora));
        fs.gravar_novo(&format!("rooms/darius-teste/presence/{uid}"), presenca).await.unwrap();

        let lista = listar(&fs).await.unwrap();
        let eu = lista.iter().find(|u| u.uid == uid).expect("jogador na lista");
        assert_eq!((eu.status.as_str(), eu.activity.as_deref()), ("away", Some("Darius Gaiden")));
        assert_eq!(eu.display_name.as_deref(), Some("Piloto"));
        assert_eq!(eu.moderation, Moderacao::default());

        let admin = FirebaseUser { uid: "admin-uid".into(), email: Some("admin@x".into()), email_verified: true };
        assert!(bloqueio(&fs, &uid).await.unwrap().is_none());
        let m = moderar(&fs, &uid, ModeracaoEntrada { banned: None, muted: Some(true), disabled: None, chat_hidden: Some(true), note: Some("spam".into()) }, &admin).await.unwrap();
        assert!(m.muted && m.chat_hidden && !m.banned);
        assert!(bloqueio(&fs, &uid).await.unwrap().is_none(), "mudo ainda joga");
        let ocultos = fs.ler_documento("chatModeration/hidden").await.unwrap().unwrap();
        assert!(crate::firestore::valor::ler_lista_de_textos(&ocultos, "uids").contains(&uid));

        // Só o campo enviado muda; o motivo anterior fica.
        let m = moderar(&fs, &uid, ModeracaoEntrada { banned: None, muted: None, disabled: Some(true), chat_hidden: Some(false), note: None }, &admin).await.unwrap();
        assert!(m.muted && m.disabled && !m.chat_hidden);
        assert_eq!(m.note.as_deref(), Some("spam"));
        assert!(bloqueio(&fs, &uid).await.unwrap().is_some());
        let ocultos = fs.ler_documento("chatModeration/hidden").await.unwrap().unwrap();
        assert!(!crate::firestore::valor::ler_lista_de_textos(&ocultos, "uids").contains(&uid));

        assert!(moderar(&fs, "admin-uid", ModeracaoEntrada { banned: Some(true), muted: None, disabled: None, chat_hidden: None, note: None }, &admin).await.is_err(), "não modera a si mesmo");
        assert!(moderar(&fs, "ninguem-xyz", ModeracaoEntrada { banned: Some(true), muted: None, disabled: None, chat_hidden: None, note: None }, &admin).await.is_err());
    }

    #[test]
    fn moderacao_sem_campos_e_liberada() {
        assert_eq!(moderacao_de(&Campos::new()), Moderacao::default());
        let mut c = Campos::new();
        c.insert("muted".into(), booleano(true));
        c.insert("note".into(), texto("spam"));
        let m = moderacao_de(&c);
        assert!(m.muted && !m.banned && !m.disabled && !m.chat_hidden);
        assert_eq!(m.note.as_deref(), Some("spam"));
    }
}
