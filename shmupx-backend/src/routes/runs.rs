use std::sync::Arc;

use axum::http::{HeaderMap, StatusCode};
use axum::middleware;
use axum::routing::post;
use axum::{Extension, Json, Router};
use serde::Deserialize;

use crate::error::AppError;
use crate::firestore::Firestore;
use crate::middleware::firebase::FirebaseUser;
use crate::middleware::jogador::{exigir_jogador, Origem};
use crate::routes::notificacoes::versao_do_client;
use crate::services::gameplays::{self, Detalhes, Resultado, RunEncerrada, RunIniciada, RunSubmetida};
use crate::services::{usuarios, versoes};

/// O jogador vem sempre da credencial (`Extension<FirebaseUser>`), nunca do payload: ID token do
/// Firebase (Client de PC) ou token do MiSTer (`middleware::jogador`, ADR 008).
/// `route_layer` escopa a exigência só a estas rotas, sem afetar `/health` após o `.merge()`.
pub fn router<S>(firestore: Arc<Firestore>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/runs/start", post(iniciar_run))
        .route("/api/runs/end", post(encerrar_run))
        .route("/api/runs", post(submeter_run))
        .route_layer(middleware::from_fn(exigir_jogador))
        .layer(Extension(firestore))
}

#[derive(Debug, Deserialize)]
struct IniciarRunPayload {
    jogo_id: String,
    /// `highscore`, `1cc`, `caravan-sprint`, `caravan-classic`. Opcional (Client antigo).
    #[serde(default)]
    modo: Option<String>,
    /// Versão do jogo; o ranking separa versões diferentes do mesmo jogo.
    #[serde(default)]
    versao: Option<String>,
}

/// `POST /api/runs/start`: debita 1 ficha e abre a gameplay (ver `services::gameplays`). Antes,
/// recusa Clients abaixo de uma versão obrigatória já liberada (sem debitar a ficha).
async fn iniciar_run(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Extension(Origem(origem)): Extension<Origem>,
    headers: HeaderMap,
    Json(payload): Json<IniciarRunPayload>,
) -> Result<(StatusCode, Json<RunIniciada>), AppError> {
    // Conta banida ou desativada no Users Manager do Lab não gasta ficha nem grava run.
    if let Some(motivo) = usuarios::bloqueio(&firestore, &usuario.uid).await? {
        return Err(AppError::forbidden(motivo));
    }
    // A versão obrigatória vale para o Client de PC; o MiSTer não manda versão de Client.
    if origem == "pc" {
        versoes::exigir_versao_em_dia(&firestore, versao_do_client(&headers).as_deref()).await?;
    }
    let detalhes = Detalhes { modo: payload.modo, versao: payload.versao, origem };
    let run = gameplays::iniciar(&firestore, &usuario.uid, &payload.jogo_id, &detalhes).await?;
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
    /// Opcionais: o MiSTer só lê o placar.
    #[serde(default)]
    vidas_perdidas: Option<i32>,
    #[serde(default)]
    continues_usados: Option<i32>,
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
