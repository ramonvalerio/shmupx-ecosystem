//! Autenticação das rotas de partida: login do Firebase (Client de PC) **ou** token do MiSTer
//! (`Bearer sxm_...`, ver `services::mister`). Nos dois casos o handler recebe o mesmo
//! `FirebaseUser` (só o `uid` importa ali) e uma `Origem`, que vai gravada na gameplay.

use std::sync::Arc;

use axum::extract::Request;
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;

use crate::error::AppError;
use crate::firestore::Firestore;
use crate::middleware::auth::extrair_bearer_token;
use crate::middleware::firebase::{verificar_requisicao, FirebaseUser};
use crate::services::mister;

/// De onde veio a partida. Decidido pelo tipo de credencial, nunca pelo corpo da requisição.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Origem(pub &'static str);

/// Exige `Authorization: Bearer <ID token do Firebase | token do MiSTer>`. Precisa do
/// `Extension<Arc<Firestore>>` aplicado por fora (para conferir o token do MiSTer).
pub async fn exigir_jogador(mut req: Request, next: Next) -> Result<Response, AppError> {
    let cabecalho = req.headers().get(AUTHORIZATION).and_then(|valor| valor.to_str().ok());
    let token = extrair_bearer_token(cabecalho)?;
    let (usuario, origem) = if mister::e_token_do_mister(token) {
        let firestore = req
            .extensions()
            .get::<Arc<Firestore>>()
            .cloned()
            .ok_or_else(|| AppError::internal("Firestore indisponível para conferir o token do MiSTer"))?;
        let uid = mister::dono(&firestore, token)
            .await?
            .ok_or_else(|| AppError::unauthorized("token do MiSTer inválido ou revogado"))?;
        (FirebaseUser { uid, email: None, email_verified: false }, Origem("mister"))
    } else {
        (verificar_requisicao(req.headers()).await?, Origem("pc"))
    };
    req.extensions_mut().insert(usuario);
    req.extensions_mut().insert(origem);
    Ok(next.run(req).await)
}
