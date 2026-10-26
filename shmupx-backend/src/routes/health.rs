use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

/// Endpoint de verificação de integridade: `GET /health` → `{"status": "ok"}`.
///
/// Genérico sobre `S` (em vez de `Router<PgPool>` fixo) porque este handler não precisa de
/// estado nenhum; isso permite `.merge()` com `routes::jogos::router()` (que é `Router<PgPool>`)
/// sem o main.rs precisar converter os dois para o mesmo tipo manualmente.
pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new().route("/health", get(health))
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
