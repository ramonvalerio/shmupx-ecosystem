use std::sync::Arc;

use axum::extract::Path;
use axum::middleware;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};

use crate::error::AppError;
use crate::firestore::Firestore;
use crate::middleware::firebase::{exigir_usuario_firebase, FirebaseUser};
use crate::services::eventos::{self, EventoDoJogador, Inscricao};
use crate::services::usuarios;

/// Eventos no Client (jogador logado): o carrossel da sala e a inscrição paga em fichas. O jogador
/// vem sempre do ID token, nunca do corpo.
pub fn router<S>(firestore: Arc<Firestore>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/events", get(listar))
        .route("/api/events/{id}/join", post(participar))
        .route_layer(middleware::from_fn(exigir_usuario_firebase))
        .layer(Extension(firestore))
}

/// `GET /api/events`: eventos publicados que ainda não terminaram, com a inscrição do jogador.
async fn listar(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
) -> Result<Json<Vec<EventoDoJogador>>, AppError> {
    Ok(Json(eventos::listar_para_jogador(&firestore, &usuario.uid).await?))
}

/// `POST /api/events/{id}/join`: inscreve o jogador, debitando as fichas do evento.
async fn participar(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
) -> Result<Json<Inscricao>, AppError> {
    // Conta banida ou desativada no Users Manager do Lab não gasta fichas.
    if let Some(motivo) = usuarios::bloqueio(&firestore, &usuario.uid).await? {
        return Err(AppError::forbidden(motivo));
    }
    let inscricao = eventos::participar(&firestore, &usuario.uid, &id).await?;
    println!("eventos: {} inscrito em {} (saldo {:?})", usuario.uid, inscricao.event.id, inscricao.balance);
    Ok(Json(inscricao))
}
