use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use sqlx::PgPool;

use crate::error::AppError;
use crate::models::Jogo;

/// `GET /api/jogos`: catálogo de shmups cadastrados (ver `public.jogos` na migration).
pub fn router() -> Router<PgPool> {
    Router::new().route("/api/jogos", get(listar_jogos))
}

async fn listar_jogos(State(pool): State<PgPool>) -> Result<Json<Vec<Jogo>>, AppError> {
    let jogos = sqlx::query_as!(
        Jogo,
        "SELECT id, steam_app_id, nome, criado_em FROM public.jogos ORDER BY nome"
    )
    .fetch_all(&pool)
    .await?;

    Ok(Json(jogos))
}
