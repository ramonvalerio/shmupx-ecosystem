use std::time::Duration;

use serde::{Deserialize, Serialize};

/// `shmupx-backend` (Axum). Sem variável de ambiente ainda — porta fixa de dev, igual ao que
/// `main.rs` do backend escuta.
const BACKEND_BASE_URL: &str = "http://localhost:3000";

/// Generoso o bastante para uma query de banco lenta, mas finito: uma chamada de rede travada
/// não deve travar o app do jogador indefinidamente (ex.: backend fora do ar).
const TIMEOUT: Duration = Duration::from_secs(10);

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("falha ao construir cliente HTTP: {e}"))
}

#[derive(Debug, Serialize)]
struct IniciarRunPayload<'a> {
    jogo_id: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IniciarRunResponse {
    pub run_id: String,
    pub server_started_at: String,
    pub max_duration_segundos: i32,
}

/// `POST /api/runs/start`: débito atômico de ficha no backend, abre a "ficha ativa" da run.
/// Chamada por `capture::commands::iniciar_corrida_safe` antes de sequer abrir o FFmpeg — se o
/// backend recusar (sem fichas, jogador/jogo inexistente, token inválido), a run nem começa a
/// gravar localmente.
pub async fn iniciar_run(token: &str, jogo_id: &str) -> Result<IniciarRunResponse, String> {
    let response = client()?
        .post(format!("{BACKEND_BASE_URL}/api/runs/start"))
        .bearer_auth(token)
        .json(&IniciarRunPayload { jogo_id })
        .send()
        .await
        .map_err(|e| format!("falha de rede ao iniciar a run no backend: {e}"))?;

    responder_ou_erro(response).await
}

#[derive(Debug, Serialize)]
struct SubmeterRunPayload<'a> {
    jogo_id: &'a str,
    score_bruto: i64,
    vidas_perdidas: i32,
    continues_usados: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubmeterRunResponse {
    pub run_id: String,
    pub status: String,
    pub nota_normalizada: f64,
}

/// `POST /api/runs`: submete o resultado da run (fim do timer seguro ou parada manual).
///
/// Recebe somente métricas medidas pelo validador offline e aprovadas pelo contrato local.
pub async fn submeter_run(
    token: &str,
    jogo_id: &str,
    score_bruto: i64,
    vidas_perdidas: i32,
    continues_usados: i32,
) -> Result<SubmeterRunResponse, String> {
    let response = client()?
        .post(format!("{BACKEND_BASE_URL}/api/runs"))
        .bearer_auth(token)
        .json(&SubmeterRunPayload {
            jogo_id,
            score_bruto,
            vidas_perdidas,
            continues_usados,
        })
        .send()
        .await
        .map_err(|e| format!("falha de rede ao submeter a run: {e}"))?;

    responder_ou_erro(response).await
}

async fn responder_ou_erro<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
) -> Result<T, String> {
    if !response.status().is_success() {
        let status = response.status();
        let corpo = response.text().await.unwrap_or_default();
        return Err(format!("backend respondeu HTTP {status}: {corpo}"));
    }

    response
        .json::<T>()
        .await
        .map_err(|e| format!("resposta do backend em formato inesperado: {e}"))
}
