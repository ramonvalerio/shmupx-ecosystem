use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware;
use axum::routing::post;
use axum::{Extension, Json, Router};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::jogador::exigir_jogador;
use crate::services::ranking;

/// Margem extra sobre `max_duration_segundos` para absorver latência de rede/processamento do
/// Client entre o fim do timer seguro e a chegada do POST aqui (Spec 002: "tolerância de envio").
const TOLERANCIA_ENVIO_SEGUNDOS: i64 = 120;

/// `usuario_id` nunca vem do payload — `route_layer` exige um ID token válido do Firebase em
/// ambas as rotas e injeta o UUID do perfil do jogador via `Extension<Uuid>` (ver
/// `middleware/jogador.rs`).
/// `route_layer` (em vez de `layer`) escopa isso só às rotas já registradas aqui, sem afetar o
/// resto do app depois do `.merge()` em `main.rs` (ex.: `/health`, `/api/jogos` continuam públicas).
pub fn router(pool: PgPool) -> Router<PgPool> {
    Router::new()
        .route("/api/runs/start", post(iniciar_run))
        .route("/api/runs", post(submeter_run))
        .route_layer(middleware::from_fn_with_state(pool, exigir_jogador))
}

#[derive(Debug, Deserialize)]
struct IniciarRunPayload {
    jogo_id: Uuid,
}

#[derive(Debug, Serialize)]
struct IniciarRunResponse {
    run_id: Uuid,
    server_started_at: DateTime<Utc>,
    max_duration_segundos: i32,
}

/// `POST /api/runs/start`: valida saldo de fichas, debita 1 em transação atômica e abre a
/// "ficha ativa" que `POST /api/runs` vai exigir na submissão (ver `submeter_run`).
async fn iniciar_run(
    State(pool): State<PgPool>,
    Extension(usuario_id): Extension<Uuid>,
    Json(payload): Json<IniciarRunPayload>,
) -> Result<(StatusCode, Json<IniciarRunResponse>), AppError> {
    let mut tx = pool.begin().await?;

    let fichas_disponiveis: Option<i32> = sqlx::query_scalar!(
        "SELECT fichas_disponiveis FROM public.perfis_jogadores WHERE id = $1 FOR UPDATE",
        usuario_id
    )
    .fetch_optional(&mut *tx)
    .await?;

    match fichas_disponiveis {
        Some(fichas) if fichas > 0 => {}
        Some(_) => return Err(AppError::bad_request("saldo de fichas insuficiente")),
        None => return Err(AppError::not_found("jogador não encontrado")),
    }

    let max_duration_segundos: Option<i32> = sqlx::query_scalar!(
        "SELECT max_duration_segundos FROM public.jogos WHERE id = $1",
        payload.jogo_id
    )
    .fetch_optional(&mut *tx)
    .await?;
    let max_duration_segundos =
        max_duration_segundos.ok_or_else(|| AppError::not_found("jogo não encontrado"))?;

    sqlx::query!(
        "UPDATE public.perfis_jogadores SET fichas_disponiveis = fichas_disponiveis - 1 WHERE id = $1",
        usuario_id
    )
    .execute(&mut *tx)
    .await?;

    let run = sqlx::query!(
        r#"
        INSERT INTO public.runs (jogador_id, jogo_id, max_duration_segundos)
        VALUES ($1, $2, $3)
        RETURNING id, server_started_at
        "#,
        usuario_id,
        payload.jogo_id,
        max_duration_segundos
    )
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(IniciarRunResponse {
            run_id: run.id,
            server_started_at: run.server_started_at,
            max_duration_segundos,
        }),
    ))
}

#[derive(Debug, Deserialize)]
struct SubmeterRunPayload {
    jogo_id: Uuid,
    score_bruto: i64,
    vidas_perdidas: i32,
    continues_usados: i32,
}

#[derive(Debug, Serialize)]
struct SubmeterRunResponse {
    run_id: Uuid,
    status: String,
    nota_normalizada: f64,
}

/// `POST /api/runs`: chamada pelo Client logo após a análise ONNX terminar. Antifraude rígida
/// antes de calcular qualquer nota:
/// 1. precisa existir uma ficha ativa (`status = 'ativa'`) deste jogador para este jogo —
///    sem isso, não há como saber que a run começou de verdade, então é rejeitada;
/// 2. a submissão precisa chegar dentro de `max_duration_segundos` + tolerância de envio,
///    contados do `server_started_at` — nunca do que o Client alega localmente, prevenindo
///    injeção manual de scores antigos via chamada direta à API;
/// 3. `continues_usados > 0` não invalida a run (ela ainda é registrada), mas corta a nota em
///    90% (ver `services::ranking`) — a cultura 1-Credit-Clear do ranking competitivo.
///
/// `usuario_id` vem do ID token do Firebase (`Extension<Uuid>`, injetado pelo middleware de autenticação), não
/// do corpo da requisição — ninguém consegue mais submeter (ou gastar a ficha ativa de) outro
/// jogador só sabendo o UUID dele.
async fn submeter_run(
    State(pool): State<PgPool>,
    Extension(usuario_id): Extension<Uuid>,
    Json(payload): Json<SubmeterRunPayload>,
) -> Result<Json<SubmeterRunResponse>, AppError> {
    if payload.score_bruto < 0 {
        return Err(AppError::bad_request("score_bruto não pode ser negativo"));
    }
    if payload.vidas_perdidas < 0 || payload.continues_usados < 0 {
        return Err(AppError::bad_request(
            "vidas_perdidas e continues_usados não podem ser negativos",
        ));
    }

    let mut tx = pool.begin().await?;

    let run = sqlx::query!(
        r#"
        SELECT id, server_started_at, max_duration_segundos
        FROM public.runs
        WHERE jogador_id = $1 AND jogo_id = $2 AND status = 'ativa'
        ORDER BY server_started_at DESC
        LIMIT 1
        FOR UPDATE
        "#,
        usuario_id,
        payload.jogo_id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| {
        AppError::bad_request(
            "nenhuma ficha ativa encontrada para este jogador/jogo; inicie a run via /api/runs/start",
        )
    })?;

    let decorrido = Utc::now() - run.server_started_at;
    let limite = ChronoDuration::seconds(
        run.max_duration_segundos as i64 + TOLERANCIA_ENVIO_SEGUNDOS,
    );

    if decorrido > limite {
        sqlx::query!(
            "UPDATE public.runs SET status = 'expirada', submetida_em = now() WHERE id = $1",
            run.id
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        return Err(AppError::conflict(
            "janela de envio expirada para esta run; a ficha não é devolvida",
        ));
    }

    let stats = sqlx::query!(
        r#"
        SELECT AVG(score_bruto)::float8 AS mu, STDDEV(score_bruto)::float8 AS sigma
        FROM public.runs
        WHERE jogo_id = $1 AND status = 'valida'
        "#,
        payload.jogo_id
    )
    .fetch_one(&mut *tx)
    .await?;

    let nota_base = ranking::calcular_nota(payload.score_bruto, stats.mu, stats.sigma);
    let nota_final = ranking::aplicar_penalidade_continue(nota_base, payload.continues_usados);

    sqlx::query!(
        r#"
        UPDATE public.runs
        SET status = 'valida',
            score_bruto = $1,
            vidas_perdidas = $2,
            continues_usados = $3,
            nota_normalizada = $4,
            submetida_em = now()
        WHERE id = $5
        "#,
        payload.score_bruto,
        payload.vidas_perdidas,
        payload.continues_usados,
        nota_final,
        run.id
    )
    .execute(&mut *tx)
    .await?;

    // Reconsolida a média das performances do jogador em vez de manter um contador separado:
    // sempre consistente com o histórico real em `runs`, nunca pode "dessincronizar".
    sqlx::query!(
        r#"
        UPDATE public.perfis_jogadores
        SET pontuacao_global_normalizada = (
            SELECT COALESCE(AVG(nota_normalizada), 0)
            FROM public.runs
            WHERE jogador_id = $1 AND status = 'valida'
        )
        WHERE id = $1
        "#,
        usuario_id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(SubmeterRunResponse {
        run_id: run.id,
        status: "valida".to_string(),
        nota_normalizada: nota_final,
    }))
}
