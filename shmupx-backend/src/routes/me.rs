use axum::middleware;
use axum::routing::get;
use axum::{Extension, Json, Router};
use serde_json::{json, Value};

use crate::middleware::firebase::{exigir_usuario_firebase, FirebaseUser};

/// `GET /api/me`: devolve a identidade Firebase do token. Serve para o cliente confirmar que o
/// backend aceita a sessão Google; não consulta o banco.
pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/me", get(me))
        .route_layer(middleware::from_fn(exigir_usuario_firebase))
}

async fn me(Extension(usuario): Extension<FirebaseUser>) -> Json<Value> {
    Json(json!({ "uid": usuario.uid, "email": usuario.email }))
}
