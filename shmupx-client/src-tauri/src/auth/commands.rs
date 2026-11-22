use serde::Serialize;

use super::oauth;

#[derive(Debug, Serialize)]
pub struct GoogleLogin {
    pub id_token: String,
}

/// Abre o navegador do sistema no consentimento do Google e aguarda o retorno no loopback.
/// `client_id` é o cliente OAuth "Desktop" do projeto Firebase (valor público).
#[tauri::command]
pub async fn login_google_desktop(app: tauri::AppHandle, client_id: String) -> Result<GoogleLogin, String> {
    let resultado = oauth::login(&client_id).await;
    // Deu certo ou não (cancelado, erro), o jogador precisa ver o Client de novo.
    oauth::trazer_para_frente(&app);
    Ok(GoogleLogin { id_token: resultado? })
}
