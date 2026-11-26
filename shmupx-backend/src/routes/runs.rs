use std::sync::Arc;

use axum::http::{HeaderMap, StatusCode};
use axum::middleware;
use axum::routing::post;
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::firestore::valor::ler_texto;
use crate::firestore::Firestore;
use crate::middleware::firebase::FirebaseUser;
use crate::middleware::jogador::{exigir_jogador, Origem};
use crate::routes::notificacoes::versao_do_client;
use crate::services::destaques::{self, DepoisDoEnvio};
use crate::services::gameplays::{self, Detalhes, Resultado, RunEncerrada, RunIniciada, RunSubmetida};
use crate::services::youtube::YouTube;
use crate::services::{estatisticas, jogo_aberto, recordes, usuarios, versoes};

/// O jogador vem sempre da credencial (`Extension<FirebaseUser>`), nunca do payload: ID token do
/// Firebase (Client de PC) ou token do MiSTer (`middleware::jogador`, ADR 008).
/// `route_layer` escopa a exigência só a estas rotas, sem afetar `/health` após o `.merge()`.
pub fn router<S>(firestore: Arc<Firestore>, youtube: Option<Arc<YouTube>>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/runs/start", post(iniciar_run))
        .route("/api/runs/end", post(encerrar_run))
        .route("/api/runs", post(submeter_run))
        .route("/api/runs/destaque", post(destaque_enviado))
        .route("/api/mister/playing", post(jogo_aberto_no_mister))
        .route_layer(middleware::from_fn(exigir_jogador))
        .layer(Extension(firestore))
        .layer(Extension(youtube))
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
    /// Ranked com live: id da live no YouTube que transmite a partida.
    #[serde(default)]
    live_broadcast_id: Option<String>,
    /// Partida valendo num evento em que o jogador se inscreveu.
    #[serde(default)]
    evento_id: Option<String>,
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
    let live_broadcast_id = payload.live_broadcast_id.filter(|id| !id.is_empty());
    let evento_id = payload.evento_id.filter(|id| !id.is_empty());
    let detalhes = Detalhes { modo: payload.modo, versao: payload.versao, origem, live_broadcast_id, evento_id };
    // Totais do jogador neste jogo (fichas gastas, tempo de jogo): criados antes da run nova, com o
    // que as gameplays antigas registram, e somados depois que ela começa.
    if gameplays::id_valido(&payload.jogo_id) {
        estatisticas::garantir(&firestore, &usuario.uid, &payload.jogo_id).await;
    }
    let run = gameplays::iniciar(&firestore, &usuario.uid, &payload.jogo_id, &detalhes).await?;
    estatisticas::run_comecou(&firestore, &usuario.uid, &payload.jogo_id, run.server_started_at).await;
    Ok((StatusCode::CREATED, Json(run)))
}

/// `POST /api/mister/playing` (token do MiSTer): o agente avisa que um jogo abriu (e renova a cada
/// minuto) ou que o MiSTer voltou ao menu. Marca ou libera o jogo aberto da conta.
async fn jogo_aberto_no_mister(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Json(aviso): Json<jogo_aberto::AvisoMister>,
) -> Result<Json<Option<jogo_aberto::JogoAberto>>, AppError> {
    Ok(Json(jogo_aberto::avisar(&firestore, &usuario.uid, aviso).await?))
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
    /// Opcionais: rota (letras maiúsculas seguidas) e bombas usadas, nos jogos com mapa de memória.
    #[serde(default)]
    rota: Option<String>,
    #[serde(default)]
    bombas_usadas: Option<i32>,
    /// Opcional: o jogo foi zerado (o fim do jogo). Sem ele, a run não entra na tabela 1CC.
    #[serde(default)]
    zerou: Option<bool>,
}

/// Resultado registrado, com a posição da run, o pódio da sala e o destaque (top 3 com live).
/// `online`: a run é o recorde do jogador (em alguma tabela) e ficou no ranking online; senão ela
/// vale só no ranking local do Client e saiu do banco.
#[derive(Debug, Serialize)]
struct RespostaEnvio {
    #[serde(flatten)]
    run: RunSubmetida,
    online: bool,
    #[serde(flatten)]
    depois: DepoisDoEnvio,
}

/// `POST /api/runs`: registra o resultado do validador offline na run indicada. Reenvio do mesmo
/// resultado é idempotente; a gameplay fica em `needs_review` até a aprovação do backend.
async fn submeter_run(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(youtube): Extension<Option<Arc<YouTube>>>,
    Extension(usuario): Extension<FirebaseUser>,
    Json(payload): Json<SubmeterRunPayload>,
) -> Result<Json<RespostaEnvio>, AppError> {
    let resultado = Resultado {
        score_bruto: payload.score_bruto,
        vidas_perdidas: payload.vidas_perdidas,
        continues_usados: payload.continues_usados,
        rota: payload.rota,
        bombas_usadas: payload.bombas_usadas,
        zerou: payload.zerou,
    };
    let (score, vidas) = (resultado.score_bruto, resultado.vidas_perdidas);
    let run = gameplays::submeter(&firestore, &usuario.uid, &payload.run_id, &payload.jogo_id, resultado).await?;
    // O modo da run, lido antes da poda: uma run abaixo do recorde sai do banco.
    let modo = firestore
        .ler_documento(&format!("gameplays/{}", payload.run_id))
        .await
        .ok()
        .flatten()
        .and_then(|gameplay| ler_texto(&gameplay, "mode"));
    // Ranking online só com o recorde: a run atual contra o que está no banco.
    let online = recordes::manter_so_os_recordes(&firestore, &usuario.uid, &payload.jogo_id, &payload.run_id)
        .await
        .unwrap_or_else(|e| {
            eprintln!("recordes: run {}: {e:?}", payload.run_id);
            true
        });
    let mut depois = destaques::depois_do_envio(&firestore, youtube.as_deref(), &usuario.uid, &payload.run_id, &payload.jogo_id).await;
    // Run abaixo do recorde do jogador: sem registro no ranking, mas com o RANK que o placar teria.
    if depois.posicao.is_none() && !online {
        depois.posicao = destaques::posicao_fora_do_recorde(&firestore, &usuario.uid, &payload.jogo_id, modo.as_deref(), score, vidas).await;
    }
    Ok(Json(RespostaEnvio { run, online, depois }))
}

#[derive(Debug, Deserialize)]
struct DestaqueEnviadoPayload {
    run_id: String,
    video_id: String,
}

/// `POST /api/runs/destaque`: o Client terminou de mandar a live do top 3 ao canal oficial.
async fn destaque_enviado(
    Extension(firestore): Extension<Arc<Firestore>>,
    Extension(usuario): Extension<FirebaseUser>,
    Json(payload): Json<DestaqueEnviadoPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    destaques::registrar_envio(&firestore, &usuario.uid, &payload.run_id, &payload.video_id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
