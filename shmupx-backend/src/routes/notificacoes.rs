use std::sync::Arc;

use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware;
use axum::response::{IntoResponse, Response};
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

/// Rota pública do updater do Tauri: o Client consulta antes de existir login. Só expõe o que já
/// foi liberado (versão, notas, URL pública do instalador e assinatura).
pub fn router_publico<S>(firestore: Arc<Firestore>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/client/update/{target}/{arch}/{current}", get(atualizacao))
        .route("/api/client/downloads", get(downloads))
        .route("/api/client/downloads/click", post(clique_no_download))
        .layer(Extension(firestore))
}

/// `POST /api/client/downloads/click` (público): o site avisa o clique num instalador (`sendBeacon`,
/// que pode mandar o JSON como texto). Sempre `204`; o Client Manager do Lab mostra os números.
async fn clique_no_download(
    Extension(firestore): Extension<Arc<Firestore>>,
    headers: HeaderMap,
    corpo: String,
) -> Result<StatusCode, AppError> {
    let clique: crate::services::cliques_download::Clique =
        serde_json::from_str(corpo.trim()).map_err(|_| AppError::bad_request("clique inválido"))?;
    let ip = crate::services::cliques_download::ip_do_visitante(headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()));
    crate::services::cliques_download::registrar(&firestore, &clique, &ip).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct FormatoDoPacote {
    /// `{{bundle_type}}` do updater (`nsis`, `appimage`, `deb`...). Clients anteriores à 0.1.5
    /// não enviam: no Linux, ficam sem atualização automática.
    bundle: Option<String>,
}

/// `GET /api/client/update/{target}/{arch}/{current}?bundle=...`: 200 com a versão a instalar ou
/// 204 quando o Client está em dia. Windows recebe o instalador NSIS; Linux, o pacote do mesmo
/// formato que tem instalado (AppImage ou .deb).
async fn atualizacao(
    Extension(firestore): Extension<Arc<Firestore>>,
    Path((target, _arch, atual)): Path<(String, String, String)>,
    Query(formato): Query<FormatoDoPacote>,
) -> Result<Response, AppError> {
    let Some(plataforma) = versoes::Plataforma::de(&target, formato.bundle.as_deref()) else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    let lista = versoes::listar(&firestore).await?;
    Ok(match versoes::atualizacao(&lista, &atual, &plataforma) {
        Some(atualizacao) => Json(atualizacao).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    })
}

/// `GET /api/client/downloads` (público): instaladores da página de Download do site, da versão
/// mais nova liberada de cada sistema.
async fn downloads(Extension(firestore): Extension<Arc<Firestore>>) -> Result<Json<versoes::Downloads>, AppError> {
    Ok(Json(versoes::downloads(&versoes::listar(&firestore).await?)))
}

/// `GET /api/notifications`: avisos com o contador de não lidos. Registra o jogador (e-mail e
/// versão do Client) para os avisos por e-mail.
async fn listar(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    headers: HeaderMap,
) -> Result<Json<ListaNotificacoes>, AppError> {
    let jogador = notificacoes::registrar_jogador(&firestore, &usuario, versao_do_client(&headers).as_deref()).await?;
    Ok(Json(notificacoes::listar(&firestore, &usuario.uid, &jogador).await?))
}

/// `GET /api/notifications/{id}`: conteúdo completo; abrir marca como lida.
async fn detalhe(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
) -> Result<Json<Notificacao>, AppError> {
    let jogador = firestore.ler_documento(&format!("players/{}", usuario.uid)).await?.unwrap_or_default();
    let mut notificacao = notificacoes::obter(&firestore, &usuario.uid, &jogador, &id).await?;
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
    let ids: Vec<String> = notificacoes::listar(&firestore, &usuario.uid, &jogador).await?.items.into_iter().map(|n| n.id).collect();
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
