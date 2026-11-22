//! Rotas do YouTube do jogador (ver `services::youtube`). O jogador vem sempre do ID token; o app
//! nunca recebe o token do Google, só o endereço de envio da live e o link.

use std::sync::Arc;

use axum::extract::Path;
use axum::middleware;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::firestore::Firestore;
use crate::middleware::firebase::{exigir_usuario_firebase, FirebaseUser};
use crate::services::youtube::{self, Conexao, LiveCriada, Transicao, YouTube};

pub fn router<S>(firestore: Arc<Firestore>, youtube: Option<Arc<YouTube>>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/youtube/config", get(config))
        .route("/api/youtube/connection", get(conexao).delete(desconectar))
        .route("/api/youtube/connect", post(conectar))
        .route("/api/live/start", post(iniciar))
        .route("/api/live/streams/{id}/status", get(situacao))
        .route("/api/live/broadcasts/{id}/transition", post(transicao))
        .route("/api/live/broadcasts/{id}/viewers", get(espectadores))
        .route_layer(middleware::from_fn(exigir_usuario_firebase))
        .layer(Extension(firestore))
        .layer(Extension(youtube))
}

fn configurado(youtube: Option<Arc<YouTube>>) -> Result<Arc<YouTube>, AppError> {
    youtube.ok_or_else(|| AppError::unavailable("Integração com o YouTube não configurada neste servidor."))
}

#[derive(Serialize)]
struct Config {
    client_id: String,
    scope: &'static str,
}

/// Cliente OAuth e escopo para o app abrir o consentimento do YouTube.
async fn config(Extension(youtube): Extension<Option<Arc<YouTube>>>) -> Result<Json<Config>, AppError> {
    let youtube = configurado(youtube)?;
    Ok(Json(Config { client_id: youtube.client_id.clone(), scope: youtube::ESCOPO }))
}

async fn conexao(
    Extension(fs): Extension<Arc<Firestore>>,
    Extension(youtube): Extension<Option<Arc<YouTube>>>,
    Extension(usuario): Extension<FirebaseUser>,
) -> Result<Json<Conexao>, AppError> {
    Ok(Json(configurado(youtube)?.conexao(&fs, &usuario.uid).await?))
}

#[derive(Deserialize)]
struct ConectarPayload {
    code: String,
    code_verifier: String,
    redirect_uri: String,
}

async fn conectar(
    Extension(fs): Extension<Arc<Firestore>>,
    Extension(youtube): Extension<Option<Arc<YouTube>>>,
    Extension(usuario): Extension<FirebaseUser>,
    Json(p): Json<ConectarPayload>,
) -> Result<Json<Conexao>, AppError> {
    let loopback = p.redirect_uri.strip_prefix("http://127.0.0.1:").and_then(|porta| porta.parse::<u16>().ok()).is_some();
    if !loopback || p.code.is_empty() || p.code_verifier.is_empty() {
        return Err(AppError::bad_request("pedido de conexão inválido"));
    }
    let conexao = configurado(youtube)?.conectar(&fs, &usuario.uid, &p.code, &p.code_verifier, &p.redirect_uri).await?;
    println!("youtube: {} conectou o canal", usuario.uid);
    Ok(Json(conexao))
}

async fn desconectar(
    Extension(fs): Extension<Arc<Firestore>>,
    Extension(youtube): Extension<Option<Arc<YouTube>>>,
    Extension(usuario): Extension<FirebaseUser>,
) -> Result<Json<Conexao>, AppError> {
    configurado(youtube)?.desconectar(&fs, &usuario.uid).await?;
    Ok(Json(Conexao { connected: false, channel_title: None }))
}

#[derive(Deserialize)]
struct IniciarPayload {
    title: String,
    #[serde(default)]
    public: bool,
}

async fn iniciar(
    Extension(fs): Extension<Arc<Firestore>>,
    Extension(youtube): Extension<Option<Arc<YouTube>>>,
    Extension(usuario): Extension<FirebaseUser>,
    Json(p): Json<IniciarPayload>,
) -> Result<Json<LiveCriada>, AppError> {
    let live = configurado(youtube)?.criar_live(&fs, &usuario.uid, &p.title, p.public).await?;
    println!("youtube: {} criou a live {} (public={})", usuario.uid, live.broadcast_id, p.public);
    Ok(Json(live))
}

#[derive(Serialize)]
struct Situacao {
    status: String,
}

async fn situacao(
    Extension(fs): Extension<Arc<Firestore>>,
    Extension(youtube): Extension<Option<Arc<YouTube>>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
) -> Result<Json<Situacao>, AppError> {
    if !youtube::id_valido(&id) {
        return Err(AppError::bad_request("id inválido"));
    }
    Ok(Json(Situacao { status: configurado(youtube)?.situacao_do_stream(&fs, &usuario.uid, &id).await? }))
}

#[derive(Serialize)]
struct Espectadores {
    viewers: u64,
}

/// `GET /api/live/broadcasts/{id}/viewers`: quantos assistem à live do próprio jogador agora (o
/// token do YouTube é o dele, então só vale para as lives do canal dele).
async fn espectadores(
    Extension(fs): Extension<Arc<Firestore>>,
    Extension(youtube): Extension<Option<Arc<YouTube>>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
) -> Result<Json<Espectadores>, AppError> {
    if !youtube::id_valido(&id) {
        return Err(AppError::bad_request("id inválido"));
    }
    Ok(Json(Espectadores { viewers: configurado(youtube)?.espectadores(&fs, &usuario.uid, &id).await? }))
}

#[derive(Deserialize)]
struct TransicaoPayload {
    status: Transicao,
}

async fn transicao(
    Extension(fs): Extension<Arc<Firestore>>,
    Extension(youtube): Extension<Option<Arc<YouTube>>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
    Json(p): Json<TransicaoPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    if !youtube::id_valido(&id) {
        return Err(AppError::bad_request("id inválido"));
    }
    configurado(youtube)?.transicao(&fs, &usuario.uid, &id, p.status).await?;
    println!("youtube: {} levou a live {id} para {:?}", usuario.uid, p.status);
    Ok(Json(serde_json::json!({ "ok": true })))
}
