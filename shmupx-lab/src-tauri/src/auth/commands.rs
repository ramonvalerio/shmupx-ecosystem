use serde::Serialize;

use super::oauth;

#[derive(Debug, Serialize)]
pub struct GoogleLogin {
    pub id_token: String,
}

/// Abre o navegador do sistema no consentimento do Google e aguarda o retorno no loopback.
/// `client_id` é o cliente OAuth "Desktop" do projeto Firebase (valor público).
/// `backend_url` é o backend do ambiente escolhido ao abrir o Lab (Dev ou Prod); sem ele, vale o
/// padrão da build (`SHMUPX_BACKEND_URL`).
#[tauri::command]
pub async fn login_google_desktop(client_id: String, backend_url: Option<String>) -> Result<GoogleLogin, String> {
    let id_token = oauth::login(&client_id, backend_url.as_deref()).await?;
    Ok(GoogleLogin { id_token })
}
