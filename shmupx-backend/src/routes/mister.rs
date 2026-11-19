use std::sync::Arc;

use axum::middleware;
use axum::routing::post;
use axum::{Extension, Json, Router};
use serde_json::{json, Value};

use crate::error::AppError;
use crate::firestore::Firestore;
use crate::middleware::firebase::{exigir_usuario_firebase, FirebaseUser};
use crate::services::email::Ses;
use crate::services::mister;

/// `POST /api/mister/tokens`: só com login do Firebase (não com outro token do MiSTer).
pub fn router<S>(firestore: Arc<Firestore>, email: Option<Arc<Ses>>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/mister/tokens", post(enviar_token))
        .route_layer(middleware::from_fn(exigir_usuario_firebase))
        .layer(Extension(firestore))
        .layer(Extension(email))
}

/// Gera um token do MiSTer e manda para o e-mail verificado da conta. A resposta diz só para
/// qual endereço (mascarado) foi: o token não volta pela API.
async fn enviar_token(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(email): Extension<Option<Arc<Ses>>>,
    Extension(usuario): Extension<FirebaseUser>,
) -> Result<Json<Value>, AppError> {
    let destino = usuario
        .email
        .clone()
        .filter(|_| usuario.email_verified)
        .ok_or_else(|| AppError::forbidden("a conta precisa de um e-mail verificado para receber o token do MiSTer"))?;
    let email = email.ok_or_else(|| AppError::unavailable("envio de e-mail não configurado no servidor"))?;

    let token = mister::gerar_token();
    mister::registrar(&firestore, &usuario.uid, &token).await?;
    let (assunto, html, texto) = mister::mensagem(&token);
    if let Err(erro) = email.enviar(std::slice::from_ref(&destino), &assunto, &html, &texto).await {
        // Token que não chegou a ninguém não pode ficar valendo.
        mister::revogar(&firestore, &token).await?;
        eprintln!("falha ao enviar o token do MiSTer: {erro}");
        return Err(AppError::unavailable("não foi possível enviar o e-mail agora; tente de novo"));
    }
    Ok(Json(json!({ "enviado_para": mister::mascarar_email(&destino) })))
}
