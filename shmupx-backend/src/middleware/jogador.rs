use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::firebase::verificar_requisicao;

/// Saldo de fichas de um jogador no primeiro acesso.
pub const FICHAS_INICIAIS: i32 = 50;

/// Middleware das rotas que mexem em fichas/runs: valida o ID token do Firebase, resolve (ou cria)
/// o `perfis_jogadores` correspondente ao UID e injeta o UUID do perfil como `Extension<Uuid>`,
/// o mesmo contrato que os handlers já usavam com o JWT do Supabase.
pub async fn exigir_jogador(
    State(pool): State<PgPool>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let usuario = verificar_requisicao(req.headers()).await?;
    let jogador_id = resolver_jogador(&pool, &usuario.uid).await?;
    req.extensions_mut().insert(jogador_id);
    req.extensions_mut().insert(usuario);
    Ok(next.run(req).await)
}

/// Busca o perfil pelo UID; se não existir, cria com `FICHAS_INICIAIS`. `ON CONFLICT DO NOTHING`
/// cobre duas requisições simultâneas do mesmo jogador novo: a perdedora relê o perfil criado.
async fn resolver_jogador(pool: &PgPool, firebase_uid: &str) -> Result<Uuid, AppError> {
    const SELECIONAR: &str = "SELECT id FROM public.perfis_jogadores WHERE firebase_uid = $1";

    if let Some(id) = sqlx::query_scalar(SELECIONAR).bind(firebase_uid).fetch_optional(pool).await? {
        return Ok(id);
    }

    let criado: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO public.perfis_jogadores (firebase_uid, fichas_disponiveis) VALUES ($1, $2) \
         ON CONFLICT (firebase_uid) DO NOTHING RETURNING id",
    )
    .bind(firebase_uid)
    .bind(FICHAS_INICIAIS)
    .fetch_optional(pool)
    .await?;

    match criado {
        Some(id) => Ok(id),
        None => Ok(sqlx::query_scalar(SELECIONAR).bind(firebase_uid).fetch_one(pool).await?),
    }
}
