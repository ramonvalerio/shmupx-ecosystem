use std::time::Duration;

use serde::{Deserialize, Serialize};

/// `shmupx-backend` padrão, usado quando o React não informa o endereço. O build de distribuição
/// pode definir `SHMUPX_BACKEND_URL`; sem ela, aponta para o backend local de desenvolvimento.
/// No Lab o React sempre envia o backend do ambiente escolhido (Dev ou Prod) ao abrir o app.
const BACKEND_BASE_URL: &str = match option_env!("SHMUPX_BACKEND_URL") {
    Some(url) => url,
    None => "http://localhost:3000",
};

const TIMEOUT: Duration = Duration::from_secs(10);

/// Endereço informado em tempo de execução, se for http(s); senão, o padrão da build.
fn base_url(informado: Option<&str>) -> Result<&str, String> {
    match informado.map(|u| u.trim().trim_end_matches('/')).filter(|u| !u.is_empty()) {
        None => Ok(BACKEND_BASE_URL),
        Some(url) if url.starts_with("https://") || url.starts_with("http://") => Ok(url),
        Some(url) => Err(format!("endereço do backend inválido: {url}")),
    }
}

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
    backend_url: Option<&str>,
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
    client_id: &str,
) -> Result<String, String> {
    let base = base_url(backend_url)?;
    let response = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("falha ao construir cliente HTTP: {e}"))?
        .post(format!("{base}/api/auth/google/token"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_usa_o_informado_ou_o_padrao() {
        assert_eq!(base_url(None).unwrap(), BACKEND_BASE_URL);
        assert_eq!(base_url(Some("  ")).unwrap(), BACKEND_BASE_URL);
        assert_eq!(base_url(Some("https://exemplo.run.app/")).unwrap(), "https://exemplo.run.app");
        assert_eq!(base_url(Some("http://localhost:3000")).unwrap(), "http://localhost:3000");
        assert!(base_url(Some("file:///etc")).is_err());
    }
}
