use std::sync::Arc;

use axum::extract::Path;
use axum::http::HeaderMap;
use axum::middleware;
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router};
use serde::Deserialize;

use crate::error::AppError;
use crate::firestore::Firestore;
use crate::middleware::firebase::{exigir_usuario_firebase, FirebaseUser};
use crate::services::notificacoes::{self, ListaNotificacoes, Notificacao};
use crate::services::versoes::{self, SituacaoClient};

/// Versão do Client que fez a chamada (o Client envia em toda requisição).
pub const CABECALHO_VERSAO: &str = "x-shmupx-client-version";

pub fn versao_do_client(headers: &HeaderMap) -> Option<String> {
    headers.get(CABECALHO_VERSAO).and_then(|v| v.to_str().ok()).map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Rotas do Client (jogador logado): sininho, detalhe, lidas, preferência de e-mail e situação
/// da versão. O jogador vem sempre do ID token, nunca do corpo.
pub fn router<S>(firestore: Arc<Firestore>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/notifications", get(listar))
        .route("/api/notifications/read-all", post(marcar_todas))
        .route("/api/notifications/{id}", get(detalhe))
        .route("/api/notifications/{id}/read", post(marcar_lida))
        .route("/api/me/notification-preferences", put(preferencias))
        .route("/api/client/version", get(situacao_da_versao))
        .route_layer(middleware::from_fn(exigir_usuario_firebase))
        .layer(Extension(firestore))
}

/// `GET /api/notifications`: avisos com o contador de não lidos. Registra o jogador (e-mail e
/// versão do Client) para os avisos por e-mail.
async fn listar(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    headers: HeaderMap,
) -> Result<Json<ListaNotificacoes>, AppError> {
    let jogador = notificacoes::registrar_jogador(&firestore, &usuario, versao_do_client(&headers).as_deref()).await?;
    Ok(Json(notificacoes::listar(&firestore, &jogador).await?))
}

/// `GET /api/notifications/{id}`: conteúdo completo; abrir marca como lida.
async fn detalhe(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
) -> Result<Json<Notificacao>, AppError> {
    let jogador = firestore.ler_documento(&format!("players/{}", usuario.uid)).await?.unwrap_or_default();
    let mut notificacao = notificacoes::obter(&firestore, &jogador, &id).await?;
    if !notificacao.resumo.read {
        notificacoes::marcar_lidas(&firestore, &usuario.uid, std::slice::from_ref(&id)).await?;
        notificacao.resumo.read = true;
    }
    Ok(Json(notificacao))
}

async fn marcar_lida(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    notificacoes::marcar_lidas(&firestore, &usuario.uid, &[id]).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// `POST /api/notifications/read-all`: marca como lidas todas as visíveis agora.
async fn marcar_todas(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
) -> Result<Json<serde_json::Value>, AppError> {
    let jogador = firestore.ler_documento(&format!("players/{}", usuario.uid)).await?.unwrap_or_default();
    let ids: Vec<String> = notificacoes::listar(&firestore, &jogador).await?.items.into_iter().map(|n| n.id).collect();
    notificacoes::marcar_lidas(&firestore, &usuario.uid, &ids).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreferenciasPayload {
    email_updates: bool,
}

/// `PUT /api/me/notification-preferences`: liga/desliga os avisos por e-mail.
async fn preferencias(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Json(payload): Json<PreferenciasPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    notificacoes::definir_preferencia_email(&firestore, &usuario.uid, payload.email_updates).await?;
    Ok(Json(serde_json::json!({ "email_updates": payload.email_updates })))
}

/// `GET /api/client/version`: versão mais nova liberada, próxima agendada e se a versão do
/// Client que chama precisa ser atualizada antes de jogar.
async fn situacao_da_versao(
    Extension(firestore): Extension<Arc<Firestore>>,
    headers: HeaderMap,
) -> Result<Json<SituacaoClient>, AppError> {
    let lista = versoes::listar(&firestore).await?;
    Ok(Json(versoes::situacao(&lista, versao_do_client(&headers).as_deref())))
}
