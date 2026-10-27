use std::time::Duration;

use serde::{Deserialize, Serialize};

/// `shmupx-backend`. O build de distribuição define `SHMUPX_BACKEND_URL` com o endereço
/// publicado; sem ela, aponta para o backend local de desenvolvimento.
const BACKEND_BASE_URL: &str = match option_env!("SHMUPX_BACKEND_URL") {
    Some(url) => url,
    None => "http://localhost:3000",
};

const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Serialize)]
struct TrocaCodigoPayload<'a> {
    code: &'a str,
    code_verifier: &'a str,
    redirect_uri: &'a str,
    client_id: &'a str,
}

#[derive(Debug, Deserialize)]
struct TrocaCodigoResponse {
    id_token: String,
}

/// `POST /api/auth/google/token`: o backend troca o código do login Google pelo `id_token`
/// usando o `client_secret`, que não é distribuído com o app. `client_id` diz qual client do
/// Google iniciou o login (o do Lab, distinto do client do jogador).
pub async fn trocar_codigo_google(
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
    client_id: &str,
) -> Result<String, String> {
    let response = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("falha ao construir cliente HTTP: {e}"))?
        .post(format!("{BACKEND_BASE_URL}/api/auth/google/token"))
        .json(&TrocaCodigoPayload { code, code_verifier, redirect_uri, client_id })
        .send()
        .await
        .map_err(|e| format!("falha de rede ao concluir o login (backend fora do ar?): {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let corpo = response.text().await.unwrap_or_default();
        return Err(format!("backend respondeu HTTP {status}: {corpo}"));
    }

    response
        .json::<TrocaCodigoResponse>()
        .await
        .map(|r| r.id_token)
        .map_err(|e| format!("resposta do backend em formato inesperado: {e}"))
}
