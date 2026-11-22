use std::time::Duration;

use serde::{Deserialize, Serialize};

/// `shmupx-backend` (Axum). O build de distribuição define `SHMUPX_BACKEND_URL` com o endereço
/// publicado; sem ela, aponta para o backend local de desenvolvimento.
const BACKEND_BASE_URL: &str = match option_env!("SHMUPX_BACKEND_URL") {
    Some(url) => url,
    None => "http://localhost:3000",
};

/// Generoso o bastante para uma query de banco lenta, mas finito: uma chamada de rede travada
/// não deve travar o app do jogador indefinidamente (ex.: backend fora do ar).
const TIMEOUT: Duration = Duration::from_secs(10);

/// Versão deste Client, enviada em toda chamada: o backend recusa runs de versões abaixo de uma
/// atualização obrigatória já liberada e mostra a situação da versão no sininho.
pub const VERSAO_CLIENT: &str = env!("CARGO_PKG_VERSION");

fn client() -> Result<reqwest::Client, String> {
    let mut cabecalhos = reqwest::header::HeaderMap::new();
    cabecalhos.insert(
        "x-shmupx-client-version",
        reqwest::header::HeaderValue::from_static(VERSAO_CLIENT),
    );
    reqwest::Client::builder()
        .timeout(TIMEOUT)
        .default_headers(cabecalhos)
        .build()
        .map_err(|e| format!("falha ao construir cliente HTTP: {e}"))
}

/// `GET`/`POST`/`PUT` genérico em nome do jogador, devolvendo o JSON do backend como veio (as
/// notificações só passam pelo Rust porque é ele que guarda o token).
pub async fn chamar_json(
    token: &str,
    metodo: reqwest::Method,
    caminho: &str,
    corpo: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let sem_corpo_permitido = metodo == reqwest::Method::GET || metodo == reqwest::Method::DELETE;
    let mut pedido = client()?
        .request(metodo, format!("{BACKEND_BASE_URL}{caminho}"))
        .bearer_auth(token);
    if let Some(corpo) = corpo {
        pedido = pedido.json(&corpo);
    } else if !sem_corpo_permitido {
        // POST/PUT sem corpo: o front do Cloud Run (Google) exige Content-Length (senão, HTTP 411).
        pedido = pedido.header(reqwest::header::CONTENT_LENGTH, 0).body(Vec::<u8>::new());
    }
    let response = pedido
        .send()
        .await
        .map_err(|e| format!("falha de rede com o backend: {e}"))?;
    responder_ou_erro(response).await
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
    run_id: &'a str,
    jogo_id: &'a str,
    score_bruto: i64,
    vidas_perdidas: i32,
    continues_usados: i32,
}

/// `status` é `needs_review`: o backend registra o resultado, mas a aprovação é posterior.
#[derive(Debug, Clone, Deserialize)]
pub struct SubmeterRunResponse {
    pub run_id: String,
    pub status: String,
}

/// Por que o encerramento da run falhou.
#[derive(Debug)]
pub enum FalhaEncerramento {
    /// O servidor recusou (409): a gameplay passou do tempo da ficha, ou a run já foi encerrada.
    Recusado(String),
    /// Rede ou outro erro: o resultado ainda pode ser enviado (o backend aplica a janela da gameplay).
    Outro(String),
}

#[derive(Debug, Serialize)]
struct EncerrarRunPayload<'a> {
    run_id: &'a str,
}

/// `POST /api/runs/end`: fim da gameplay (STOP ou corte da ficha). O servidor registra o fim pelo
/// próprio relógio; depois disso a análise local pode levar o tempo que precisar.
pub async fn encerrar_run(token: &str, run_id: &str) -> Result<(), FalhaEncerramento> {
    let response = client()
        .map_err(FalhaEncerramento::Outro)?
        .post(format!("{BACKEND_BASE_URL}/api/runs/end"))
        .bearer_auth(token)
        .json(&EncerrarRunPayload { run_id })
        .send()
        .await
        .map_err(|e| FalhaEncerramento::Outro(format!("falha de rede ao encerrar a run: {e}")))?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let corpo = response.text().await.unwrap_or_default();
    let detalhe = format!("backend respondeu HTTP {status}: {corpo}");
    Err(if status == reqwest::StatusCode::CONFLICT {
        FalhaEncerramento::Recusado(detalhe)
    } else {
        FalhaEncerramento::Outro(detalhe)
    })
}

/// `POST /api/runs`: submete o resultado da run (fim do timer seguro ou parada manual).
///
/// Recebe somente métricas medidas pelo validador offline e aprovadas pelo contrato local.
/// O `run_id` identifica a gameplay aberta em `/api/runs/start`; reenviar o mesmo resultado
/// é idempotente no backend.
pub async fn submeter_run(
    token: &str,
    run_id: &str,
    jogo_id: &str,
    score_bruto: i64,
    vidas_perdidas: i32,
    continues_usados: i32,
) -> Result<SubmeterRunResponse, String> {
    let response = client()?
        .post(format!("{BACKEND_BASE_URL}/api/runs"))
        .bearer_auth(token)
        .json(&SubmeterRunPayload {
            run_id,
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

#[derive(Debug, Serialize)]
struct TrocaCodigoPayload<'a> {
    code: &'a str,
    code_verifier: &'a str,
    redirect_uri: &'a str,
}

#[derive(Debug, Deserialize)]
struct TrocaCodigoResponse {
    id_token: String,
}

/// `POST /api/auth/google/token`: o backend troca o código do login Google pelo `id_token`
/// usando o `client_secret`, que não é distribuído com o app.
pub async fn trocar_codigo_google(
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> Result<String, String> {
    let response = client()?
        .post(format!("{BACKEND_BASE_URL}/api/auth/google/token"))
        .json(&TrocaCodigoPayload {
            code,
            code_verifier,
            redirect_uri,
        })
        .send()
        .await
        .map_err(|e| format!("falha de rede ao concluir o login (backend fora do ar?): {e}"))?;

    responder_ou_erro::<TrocaCodigoResponse>(response)
        .await
        .map(|r| r.id_token)
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
