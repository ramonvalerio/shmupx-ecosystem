use std::sync::Arc;

use axum::extract::Path;
use axum::middleware;
use axum::routing::{get, put};
use axum::{Extension, Json, Router};

use crate::error::AppError;
use crate::firestore::Firestore;
use crate::middleware::firebase::{exigir_usuario_firebase, FirebaseUser};
use crate::services::catalogo::{self, Jogo, JogoEntrada};

/// Rotas do Lab. Exigem ID token do Firebase e, além disso, admin (`ADMIN_EMAILS`): o servidor
/// decide a permissão a cada chamada; o app do Lab não carrega credencial nem segredo.
pub fn router<S>(firestore: Arc<Firestore>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/admin/games", get(listar_jogos))
        .route("/api/admin/games/{id}", put(salvar_jogo))
        .route_layer(middleware::from_fn(exigir_usuario_firebase))
        .layer(Extension(firestore))
}

fn exigir_admin(usuario: &FirebaseUser) -> Result<(), AppError> {
    if usuario.e_admin() {
        Ok(())
    } else {
        Err(AppError::forbidden("acesso restrito a administradores"))
    }
}

/// `GET /api/admin/games`: todos os jogos do catálogo, inclusive rascunhos.
async fn listar_jogos(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
) -> Result<Json<Vec<Jogo>>, AppError> {
    exigir_admin(&usuario)?;
    Ok(Json(catalogo::listar(&firestore).await?))
}

/// `PUT /api/admin/games/{id}`: cria ou atualiza o jogo. `published: true` o torna visível ao
/// Client e liberado para runs; `false` o esconde e bloqueia novas runs.
async fn salvar_jogo(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Path(id): Path<String>,
    Json(entrada): Json<JogoEntrada>,
) -> Result<Json<Jogo>, AppError> {
    exigir_admin(&usuario)?;
    let jogo = catalogo::salvar(&firestore, &id, entrada).await?;
    println!("catálogo: {} salvou o jogo {} (published={})", usuario.uid, jogo.id, jogo.published);
    Ok(Json(jogo))
}
