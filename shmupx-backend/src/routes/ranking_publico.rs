//! Ranking público (somente leitura, sem login): o site e o Client leem daqui. Os dados vêm das
//! gameplays no Firestore (`services::placar`); não depende do PostgreSQL legado.
//!
//! - `GET /api/ranking/jogos`: jogos publicados (id e título), para os filtros do site.
//! - `GET /api/ranking/global?limite=`: Ranking Global (Spec 002).
//! - `GET /api/ranking/jogos/{jogo_id}?modo=&versao=&limite=`: ranking de um jogo, que é também
//!   o ranking da sala (ADR 006: um jogo, uma sala).
//!
//! Entram gameplays `approved` e `needs_review` (estas marcadas `verificada: false`), só de jogos
//! publicados. Cada resposta fica 60 s em memória por instância: a rota é pública e cada consulta
//! lê as gameplays do Firestore.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Path, Query};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Json, Router};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::AppError;
use crate::firestore::valor::{ler_booleano, ler_data, ler_inteiro, ler_mapa, ler_texto};
use crate::firestore::{Campos, Firestore};
use crate::services::gameplays::id_valido;
use crate::services::placar::{self, Partida};

/// Estados de gameplay que contam para o ranking. `approved` ainda não é gravado por nenhum
/// fluxo; quando a revisão existir, basta tirar `needs_review` daqui.
const STATUS_NO_RANKING: [&str; 2] = ["approved", "needs_review"];
/// Teto de gameplays lidas por consulta. Passou disso, a resposta sai com `truncado: true` e é
/// hora de materializar o ranking (ex.: `seasons/{id}/gameRankings`).
const MAX_GAMEPLAYS: usize = 5000;
const VALIDADE_CACHE: Duration = Duration::from_secs(60);
const LIMITE_PADRAO: usize = 100;
const LIMITE_MAXIMO: usize = 500;
const MODOS: [&str; 5] = ["highscore", "1cc", "caravan", "caravan-sprint", "caravan-classic"];

#[derive(Default)]
struct Cache(Mutex<HashMap<String, (Instant, Value)>>);

impl Cache {
    fn ler(&self, chave: &str) -> Option<Value> {
        let mapa = self.0.lock().ok()?;
        mapa.get(chave).filter(|(quando, _)| quando.elapsed() < VALIDADE_CACHE).map(|(_, v)| v.clone())
    }

    fn guardar(&self, chave: String, valor: Value) {
        if let Ok(mut mapa) = self.0.lock() {
            mapa.retain(|_, (quando, _)| quando.elapsed() < VALIDADE_CACHE);
            mapa.insert(chave, (Instant::now(), valor));
        }
    }
}

pub fn router<S>(firestore: Arc<Firestore>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/ranking/jogos", get(jogos))
        .route("/api/ranking/global", get(global))
        .route("/api/ranking/jogos/{jogo_id}", get(do_jogo))
        .layer(Extension(firestore))
        .layer(Extension(Arc::new(Cache::default())))
}

#[derive(Debug, Deserialize)]
struct Filtros {
    limite: Option<usize>,
    modo: Option<String>,
    versao: Option<String>,
}

fn responder(valor: Value) -> Response {
    ([(header::CACHE_CONTROL, "public, max-age=60")], Json(valor)).into_response()
}

/// Jogos publicados, em ordem de título: o site não lê o Firestore direto (as Rules exigem login).
async fn jogos(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(cache): Extension<Arc<Cache>>,
) -> Result<Response, AppError> {
    if let Some(valor) = cache.ler("jogos") {
        return Ok(responder(valor));
    }
    let mut lista: Vec<(String, String)> = jogos_publicados(&firestore).await?.into_iter().collect();
    lista.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()).then(a.0.cmp(&b.0)));
    let valor = json!({
        "jogos": lista.into_iter().map(|(id, titulo)| json!({ "jogo_id": id, "titulo": titulo })).collect::<Vec<_>>(),
    });
    cache.guardar("jogos".into(), valor.clone());
    Ok(responder(valor))
}

async fn global(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(cache): Extension<Arc<Cache>>,
    Query(filtros): Query<Filtros>,
) -> Result<Response, AppError> {
    let limite = filtros.limite.unwrap_or(LIMITE_PADRAO).clamp(1, LIMITE_MAXIMO);
    let chave = format!("global:{limite}");
    if let Some(valor) = cache.ler(&chave) {
        return Ok(responder(valor));
    }
    let jogos = jogos_publicados(&firestore).await?;
    let (partidas, truncado) = ler_partidas(&firestore, None, &jogos).await?;
    let mut entradas = placar::ranking_global(&partidas);
    let total = entradas.len();
    entradas.truncate(limite);
    let nomes = nomes_dos_jogadores(&firestore, entradas.iter().map(|e| e.jogador_id.as_str())).await?;
    for entrada in &mut entradas {
        entrada.nome = nome_ou_padrao(&nomes, &entrada.jogador_id);
        for jogo in &mut entrada.jogos {
            jogo.titulo = jogos.get(&jogo.jogo_id).cloned().unwrap_or_else(|| jogo.jogo_id.clone());
        }
    }
    let valor = json!({
        "total": total,
        "truncado": truncado,
        "atualizado_em": Utc::now(),
        "entradas": entradas,
    });
    cache.guardar(chave, valor.clone());
    Ok(responder(valor))
}

async fn do_jogo(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(cache): Extension<Arc<Cache>>,
    Path(jogo_id): Path<String>,
    Query(filtros): Query<Filtros>,
) -> Result<Response, AppError> {
    if !id_valido(&jogo_id) {
        return Err(AppError::bad_request("jogo_id inválido"));
    }
    let modo = filtros.modo.filter(|m| !m.is_empty());
    if modo.as_deref().is_some_and(|m| !MODOS.contains(&m)) {
        return Err(AppError::bad_request("modo inválido: use highscore, 1cc ou caravan"));
    }
    let versao = filtros.versao.filter(|v| !v.is_empty());
    if versao.as_deref().is_some_and(|v| !id_valido(v.replace('.', "_").as_str())) {
        return Err(AppError::bad_request("versão inválida"));
    }
    let limite = filtros.limite.unwrap_or(LIMITE_PADRAO).clamp(1, LIMITE_MAXIMO);
    let chave = format!("jogo:{jogo_id}:{}:{}:{limite}", modo.as_deref().unwrap_or(""), versao.as_deref().unwrap_or(""));
    if let Some(valor) = cache.ler(&chave) {
        return Ok(responder(valor));
    }

    let jogos = jogos_publicados(&firestore).await?;
    let titulo = jogos.get(&jogo_id).cloned().ok_or_else(|| AppError::not_found("jogo não encontrado"))?;
    let (partidas, truncado) = ler_partidas(&firestore, Some(&jogo_id), &jogos).await?;
    let mut entradas = placar::ranking_do_jogo(&partidas, &jogo_id, modo.as_deref(), versao.as_deref());
    let total = entradas.len();
    entradas.truncate(limite);
    let nomes = nomes_dos_jogadores(&firestore, entradas.iter().map(|e| e.jogador_id.as_str())).await?;
    for entrada in &mut entradas {
        entrada.nome = nome_ou_padrao(&nomes, &entrada.jogador_id);
    }
    let valor = json!({
        "jogo_id": jogo_id,
        "titulo": titulo,
        "modo": modo,
        "versao": versao,
        "total": total,
        "truncado": truncado,
        "atualizado_em": Utc::now(),
        "entradas": entradas,
    });
    cache.guardar(chave, valor.clone());
    Ok(responder(valor))
}

/// `games` publicados: id -> título.
async fn jogos_publicados(fs: &Firestore) -> Result<HashMap<String, String>, AppError> {
    Ok(fs
        .listar("games")
        .await?
        .into_iter()
        .filter(|(_, campos)| ler_booleano(campos, "published") == Some(true))
        .map(|(id, campos)| {
            let titulo = ler_texto(&campos, "title").unwrap_or_else(|| id.clone());
            (id, titulo)
        })
        .collect())
}

fn filtro(campo: &str, op: &str, valor: Value) -> Value {
    json!({ "fieldFilter": { "field": { "fieldPath": campo }, "op": op, "value": valor } })
}

/// Gameplays com resultado nos estados do ranking (de um jogo, ou de todos), de jogos publicados.
async fn ler_partidas(
    fs: &Firestore,
    jogo: Option<&str>,
    publicados: &HashMap<String, String>,
) -> Result<(Vec<Partida>, bool), AppError> {
    let valores: Vec<Value> = STATUS_NO_RANKING.iter().map(|s| json!({ "stringValue": s })).collect();
    let status = filtro("status", "IN", json!({ "arrayValue": { "values": valores } }));
    let onde = match jogo {
        Some(jogo) => json!({ "compositeFilter": { "op": "AND", "filters": [
            status,
            filtro("gameId", "EQUAL", json!({ "stringValue": jogo })),
        ] } }),
        None => status,
    };
    let campos_lidos: Vec<Value> = ["ownerId", "gameId", "status", "mode", "gameVersion", "source", "result"]
        .iter()
        .map(|c| json!({ "fieldPath": c }))
        .collect();
    let consulta = json!({
        "from": [{ "collectionId": "gameplays" }],
        "where": onde,
        "select": { "fields": campos_lidos },
        "limit": MAX_GAMEPLAYS,
    });
    let documentos = fs.consultar(consulta).await?;
    let truncado = documentos.len() >= MAX_GAMEPLAYS;
    let partidas = documentos
        .into_iter()
        .filter_map(|(_, campos)| partida(&campos))
        .filter(|p| publicados.contains_key(&p.jogo))
        .collect();
    Ok((partidas, truncado))
}

fn partida(campos: &Campos) -> Option<Partida> {
    let resultado = ler_mapa(campos, "result")?;
    let score = ler_inteiro(&resultado, "score").filter(|s| *s >= 0)?;
    Some(Partida {
        jogador: ler_texto(campos, "ownerId")?,
        jogo: ler_texto(campos, "gameId")?,
        score,
        continues: ler_inteiro(&resultado, "continuesUsed").and_then(|c| c.try_into().ok()).unwrap_or(0),
        vidas_perdidas: ler_inteiro(&resultado, "livesLost").and_then(|v| v.try_into().ok()),
        zerou: ler_booleano(&resultado, "cleared") == Some(true),
        modo: ler_texto(campos, "mode"),
        versao: ler_texto(campos, "gameVersion"),
        origem: ler_texto(campos, "source"),
        verificada: ler_texto(campos, "status").as_deref() == Some("approved"),
        enviada_em: ler_data(&resultado, "submittedAt"),
    })
}

/// Nome de exibição: `publicProfiles/{uid}` quando existir, senão o `displayName` do perfil.
async fn nomes_dos_jogadores<'a>(
    fs: &Firestore,
    uids: impl Iterator<Item = &'a str>,
) -> Result<HashMap<String, String>, AppError> {
    let uids: HashSet<&str> = uids.collect();
    if uids.is_empty() {
        return Ok(HashMap::new());
    }
    let caminhos: Vec<String> =
        uids.iter().flat_map(|uid| [format!("publicProfiles/{uid}"), format!("users/{uid}")]).collect();
    let lidos = fs.ler_varios(&caminhos).await?;
    Ok(uids
        .into_iter()
        .filter_map(|uid| {
            let nome = [format!("publicProfiles/{uid}"), format!("users/{uid}")]
                .iter()
                .find_map(|c| lidos.get(c).and_then(|campos| ler_texto(campos, "displayName")))
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty())?;
            Some((uid.to_string(), nome))
        })
        .collect())
}

fn nome_ou_padrao(nomes: &HashMap<String, String>, uid: &str) -> String {
    nomes.get(uid).cloned().unwrap_or_else(|| "Jogador".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::firestore::valor::{data, inteiro, mapa, texto};

    fn campos(pares: &[(&str, Value)]) -> Campos {
        pares.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn gameplay_com_resultado_vira_partida() {
        let agora = Utc::now();
        let doc = campos(&[
            ("ownerId", texto("ana")),
            ("gameId", texto("earthion")),
            ("status", texto("needs_review")),
            ("mode", texto("1cc")),
            ("result", mapa(campos(&[("score", inteiro(1234)), ("continuesUsed", inteiro(0)), ("submittedAt", data(agora))]))),
        ]);
        let p = partida(&doc).expect("partida");
        assert_eq!((p.jogador.as_str(), p.jogo.as_str(), p.score, p.verificada), ("ana", "earthion", 1234, false));
        assert_eq!(p.modo.as_deref(), Some("1cc"));
    }

    #[test]
    fn gameplay_sem_resultado_fica_de_fora() {
        let doc = campos(&[("ownerId", texto("ana")), ("gameId", texto("earthion")), ("status", texto("approved"))]);
        assert!(partida(&doc).is_none());
    }

    #[test]
    fn cache_devolve_o_valor_guardado() {
        let cache = Cache::default();
        assert!(cache.ler("x").is_none());
        cache.guardar("x".into(), json!({ "ok": true }));
        assert_eq!(cache.ler("x"), Some(json!({ "ok": true })));
    }
}
