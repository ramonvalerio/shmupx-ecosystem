use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

/// Regra oficial do Ranking Global (Spec 002): soma das notas máximas (Hi-Score) apenas dos
/// 5 jogos mais jogados (por volume de runs válidas) de cada jogador — não os 5 jogos com as
/// melhores notas. Um jogador que jogou só 2 vezes um jogo em que tirou 9.9 não ganha mais
/// pontos nesse jogo do que alguém que jogou 50 vezes um jogo em que tirou 5.0, se esse último
/// estiver entre os 5 mais jogados dele e o primeiro não estiver entre os seus próprios 5 mais
/// jogados.
const MAX_JOGOS_CONSIDERADOS: i64 = 5;

pub fn router() -> Router<PgPool> {
    Router::new().route("/api/rankings/global", get(ranking_global))
}

#[derive(Debug, Serialize)]
struct RankingGlobalEntry {
    posicao: i64,
    usuario_id: Uuid,
    soma_top5_notas: f64,
    jogos_contados: i64,
}

/// `GET /api/rankings/global`: Ranking Global Oficial (Spec 002).
///
/// 1. `jogos_por_usuario`: para cada par (jogador, jogo), pega o Hi-Score (`MAX(nota_normalizada)`)
///    e quantas runs válidas o jogador tem naquele jogo (`COUNT(id)`).
/// 2. `ranqueado`: `ROW_NUMBER()` particionado por jogador, ordenado por volume de runs
///    decrescente — os jogos mais jogados de cada um ficam com `rn` menor. Critérios de
///    desempate (nota, depois jogo_id) garantem que o resultado seja sempre o mesmo, já que
///    `ROW_NUMBER()` exige uma ordenação totalmente determinística para não variar entre chamadas
///    quando dois jogos têm exatamente o mesmo número de runs.
/// 3. Query principal: filtra `rn <= 5` (só os 5 mais jogados de cada um), soma as notas máximas
///    desses jogos por jogador. Um jogador com menos de 5 jogos simplesmente soma o que tem —
///    não há divisão nem necessidade de preencher posições vazias.
async fn ranking_global(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<RankingGlobalEntry>>, AppError> {
    let linhas = sqlx::query!(
        r#"
        WITH jogos_por_usuario AS (
            SELECT
                jogador_id,
                jogo_id,
                MAX(nota_normalizada) AS maior_nota,
                COUNT(id) AS total_runs
            FROM public.runs
            WHERE status = 'valida'
            GROUP BY jogador_id, jogo_id
        ),
        ranqueado AS (
            SELECT
                jogador_id,
                maior_nota,
                ROW_NUMBER() OVER (
                    PARTITION BY jogador_id
                    ORDER BY total_runs DESC, maior_nota DESC, jogo_id
                ) AS rn
            FROM jogos_por_usuario
        )
        SELECT
            jogador_id AS "jogador_id!",
            SUM(maior_nota) AS "soma_top5_notas!",
            COUNT(*) AS "jogos_contados!",
            RANK() OVER (ORDER BY SUM(maior_nota) DESC) AS "posicao!"
        FROM ranqueado
        WHERE rn <= $1
        GROUP BY jogador_id
        ORDER BY SUM(maior_nota) DESC
        "#,
        MAX_JOGOS_CONSIDERADOS
    )
    .fetch_all(&pool)
    .await?;

    let ranking = linhas
        .into_iter()
        .map(|linha| RankingGlobalEntry {
            posicao: linha.posicao,
            usuario_id: linha.jogador_id,
            soma_top5_notas: linha.soma_top5_notas,
            jogos_contados: linha.jogos_contados,
        })
        .collect();

    Ok(Json(ranking))
}
