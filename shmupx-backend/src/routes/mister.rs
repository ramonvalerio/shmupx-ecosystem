use std::sync::Arc;

use axum::middleware;
use axum::routing::post;
use axum::{Extension, Json, Router};
use serde_json::{json, Value};

use crate::error::AppError;
use crate::firestore::Firestore;
use crate::middleware::firebase::{exigir_usuario_firebase, FirebaseUser};
use crate::services::email::Ses;
use crate::services::{mister, notificacoes};

/// `POST /api/mister/tokens`: só com login do Firebase (não com outro token do MiSTer).
pub fn router<S>(firestore: Arc<Firestore>, email: Option<Arc<Ses>>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/mister/tokens", post(enviar_token))
        .route("/api/mister/tokens/direct", post(token_direto))
        .route_layer(middleware::from_fn(exigir_usuario_firebase))
        .layer(Extension(firestore))
        .layer(Extension(email))
}

/// `POST /api/mister/tokens/direct`: gera um token e o devolve ao Client da própria conta, que o
/// grava direto no `shmupx.ini` do MiSTer pela rede (FTP). O Client não mostra o token: ele vai
/// da memória do app para o MiSTer. O aviso no sininho registra que um token novo foi criado.
async fn token_direto(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
) -> Result<Json<Value>, AppError> {
    let token = mister::gerar_token();
    mister::registrar(&firestore, &usuario.uid, &token).await?;
    let resumo = "O ShmupX Client gravou um token novo do MiSTer no seu MiSTer. Se não foi você, avise o suporte.";
    if let Err(erro) = notificacoes::notificar_jogador(&firestore, &usuario.uid, "mister", "MiSTer conectado", resumo).await {
        eprintln!("falha ao registrar o aviso do token do MiSTer no sininho: {erro:?}");
    }
    println!("mister: {} gerou um token para gravar direto no MiSTer", usuario.uid);
    Ok(Json(json!({ "token": token })))
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
    // Todo e-mail ao jogador também vira aviso no sininho do Client (sem o token, só no e-mail).
    let mascarado = mister::mascarar_email(&destino);
    let resumo = format!("Enviamos um token novo do MiSTer para {mascarado}. Se não foi você, ignore o e-mail.");
    if let Err(erro) = notificacoes::notificar_jogador(&firestore, &usuario.uid, "mister", "Token do MiSTer enviado", &resumo).await {
        // O e-mail já saiu: falhar aqui só tira o aviso do sininho.
        eprintln!("falha ao registrar o aviso do token do MiSTer no sininho: {erro:?}");
    }
    Ok(Json(json!({ "enviado_para": mascarado })))
}
