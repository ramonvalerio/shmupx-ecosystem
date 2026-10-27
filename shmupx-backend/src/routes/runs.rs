use std::sync::Arc;

use axum::http::StatusCode;
use axum::middleware;
use axum::routing::post;
use axum::{Extension, Json, Router};
use serde::Deserialize;

use crate::error::AppError;
use crate::firestore::Firestore;
use crate::middleware::firebase::{exigir_usuario_firebase, FirebaseUser};
use crate::services::gameplays::{self, Resultado, RunEncerrada, RunIniciada, RunSubmetida};

/// O jogador vem sempre do ID token do Firebase (`Extension<FirebaseUser>`), nunca do payload.
/// `route_layer` escopa a exigência só a estas rotas, sem afetar `/health` após o `.merge()`.
pub fn router<S>(firestore: Arc<Firestore>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/runs/start", post(iniciar_run))
        .route("/api/runs/end", post(encerrar_run))
        .route("/api/runs", post(submeter_run))
        .route_layer(middleware::from_fn(exigir_usuario_firebase))
        .layer(Extension(firestore))
}

#[derive(Debug, Deserialize)]
struct IniciarRunPayload {
    jogo_id: String,
}

/// `POST /api/runs/start`: debita 1 ficha e abre a gameplay (ver `services::gameplays`).
async fn iniciar_run(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Json(payload): Json<IniciarRunPayload>,
) -> Result<(StatusCode, Json<RunIniciada>), AppError> {
    let run = gameplays::iniciar(&firestore, &usuario.uid, &payload.jogo_id).await?;
    Ok((StatusCode::CREATED, Json(run)))
}

#[derive(Debug, Deserialize)]
struct EncerrarRunPayload {
    run_id: String,
}

/// `POST /api/runs/end`: fim da gameplay (STOP ou corte da ficha). O servidor registra o fim e a
/// análise local pode levar o tempo que precisar antes do envio do resultado.
async fn encerrar_run(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Json(payload): Json<EncerrarRunPayload>,
) -> Result<Json<RunEncerrada>, AppError> {
    Ok(Json(gameplays::encerrar(&firestore, &usuario.uid, &payload.run_id).await?))
}

#[derive(Debug, Deserialize)]
struct SubmeterRunPayload {
    run_id: String,
    jogo_id: String,
    score_bruto: i64,
    vidas_perdidas: i32,
    continues_usados: i32,
}

/// `POST /api/runs`: registra o resultado do validador offline na run indicada. Reenvio do mesmo
/// resultado é idempotente; a gameplay fica em `needs_review` até a aprovação do backend.
async fn submeter_run(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Json(payload): Json<SubmeterRunPayload>,
) -> Result<Json<RunSubmetida>, AppError> {
    let resultado = Resultado {
        score_bruto: payload.score_bruto,
        vidas_perdidas: payload.vidas_perdidas,
        continues_usados: payload.continues_usados,
    };
    let run = gameplays::submeter(&firestore, &usuario.uid, &payload.run_id, &payload.jogo_id, resultado).await?;
    Ok(Json(run))
}
