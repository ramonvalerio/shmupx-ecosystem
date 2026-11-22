//! Perfil público do jogador logado: nick (`@handle`) e avatar (ver `services::perfil`).

use std::sync::Arc;

use axum::extract::Path;
use axum::middleware;
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::AppError;
use crate::firestore::Firestore;
use crate::middleware::firebase::{exigir_usuario_firebase, FirebaseUser};
use crate::services::email::Ses;
use crate::services::perfil::{self, Perfil};

pub fn router<S>(firestore: Arc<Firestore>, email: Option<Arc<Ses>>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/me/profile", get(meu_perfil))
        .route("/api/me/handle", put(trocar_nick))
        .route("/api/me/handle/request", post(pedir_troca))
        .route("/api/me/avatar", put(avatar))
        .route("/api/handles/{nick}", get(disponivel))
        .route_layer(middleware::from_fn(exigir_usuario_firebase))
        .layer(Extension(firestore))
        .layer(Extension(email))
}

#[derive(Deserialize, Default)]
struct Consulta {
    #[serde(default)]
    display_name: Option<String>,
}

/// `GET /api/me/profile?display_name=...`: perfil público; na primeira vez cria o nick padrão.
async fn meu_perfil(
    Extension(fs): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    axum::extract::Query(consulta): axum::extract::Query<Consulta>,
) -> Result<Json<Perfil>, AppError> {
    let nome = consulta.display_name;
    Ok(Json(perfil::garantir(&fs, &usuario.uid, nome.as_deref(), usuario.email.as_deref()).await?))
}

/// `GET /api/handles/{nick}`: `{ valid, available }`.
async fn disponivel(
    Extension(fs): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(nick): Path<String>,
) -> Result<Json<Value>, AppError> {
    let nick = nick.trim().to_lowercase();
    let valid = perfil::nick_valido(&nick);
    Ok(Json(json!({ "valid": valid, "available": valid && perfil::disponivel(&fs, &nick, &usuario.uid).await? })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NickPayload {
    handle: String,
}

/// `PUT /api/me/handle`: a troca livre, uma vez.
async fn trocar_nick(
    Extension(fs): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Json(p): Json<NickPayload>,
) -> Result<Json<Perfil>, AppError> {
    let novo = p.handle.trim().to_lowercase();
    let perfil = perfil::trocar_nick(&fs, &usuario.uid, &novo).await?;
    println!("perfil: {} trocou o nick para {novo}", usuario.uid);
    Ok(Json(perfil))
}

/// `POST /api/me/handle/request`: depois da troca livre, pede outra ao suporte (por e-mail). Fica
/// aguardando análise; um pedido por vez.
async fn pedir_troca(
    Extension(fs): Extension<Arc<Firestore>>,
    Extension(email): Extension<Option<Arc<Ses>>>,
    Extension(usuario): Extension<FirebaseUser>,
    Json(p): Json<NickPayload>,
) -> Result<Json<Perfil>, AppError> {
    let pedido = p.handle.trim().to_lowercase();
    if !perfil::nick_valido(&pedido) {
        return Err(AppError::bad_request("nick inválido: de 3 a 20 caracteres, letras minúsculas, números e _"));
    }
    let atual = perfil::garantir(&fs, &usuario.uid, None, usuario.email.as_deref()).await?;
    if !atual.handle_locked {
        return Err(AppError::conflict("a primeira troca é livre: troque direto no perfil"));
    }
    if atual.handle_request.as_ref().is_some_and(|r| r.status == "pending") {
        return Err(AppError::conflict("já existe um pedido de troca aguardando análise"));
    }
    if !perfil::disponivel(&fs, &pedido, &usuario.uid).await? {
        return Err(AppError::conflict("esse nick já está em uso"));
    }
    let email = email.ok_or_else(|| AppError::unavailable("envio de e-mail não configurado no servidor"))?;
    let conta = usuario.email.clone().unwrap_or_else(|| "(sem e-mail)".into());
    let assunto = format!("Pedido de troca de nick: @{} -> @{pedido}", atual.handle);
    let texto = format!(
        "Pedido de troca de nick no ShmupX.\n\nJogador (uid): {}\nConta: {conta}\nNick atual: @{}\nNick pedido: @{pedido}\n\nPara aprovar, troque em publicProfiles/{{uid}} e handles/ no Firestore.",
        usuario.uid, atual.handle
    );
    let html = format!("<pre>{}</pre>", texto.replace('&', "&amp;").replace('<', "&lt;"));
    email
        .enviar(&[perfil::SUPORTE.to_string()], &assunto, &html, &texto)
        .await
        .map_err(|erro| {
            eprintln!("falha ao enviar o pedido de troca de nick: {erro}");
            AppError::unavailable("não foi possível enviar o pedido agora; tente de novo")
        })?;
    perfil::registrar_pedido(&fs, &usuario.uid, &pedido).await?;
    Ok(Json(perfil::garantir(&fs, &usuario.uid, None, None).await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AvatarPayload {
    avatar: Option<String>,
}

/// `PUT /api/me/avatar`: grava (ou remove, com `null`) o avatar recortado.
async fn avatar(
    Extension(fs): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Json(p): Json<AvatarPayload>,
) -> Result<Json<Value>, AppError> {
    perfil::definir_avatar(&fs, &usuario.uid, p.avatar.as_deref()).await?;
    Ok(Json(json!({ "ok": true })))
}
