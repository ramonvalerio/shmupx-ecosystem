//! ShmupX Lite: o app do celular só entra nas salas, conversa e assiste às lives. Tudo isso é
//! Firebase no lado da interface; o Rust conduz o login do Google e o MiSTer (rede local).

mod mister;
#[cfg(not(mobile))]
mod oauth;

use serde::Serialize;

/// Cliente OAuth "Web" do projeto (criado pelo Firebase): no Android, o login nativo do Google
/// emite o ID token para ele, e o Firebase o aceita. Público (não é credencial); o app Android é
/// reconhecido pelo cliente OAuth Android (pacote com.shmupx.lite + SHA-1 da chave de assinatura).
#[cfg(mobile)]
const CLIENTE_WEB: &str = "230979440400-0n51ccakerpdums7evc948uu7fug4opo.apps.googleusercontent.com";

#[derive(Serialize)]
struct GoogleLogin {
    id_token: String,
}

/// Mesmo nome do comando do Client (a interface reaproveitada chama este). No Android: login
/// nativo (Credential Manager, a escolha de conta abre por cima do app). No PC (desenvolvimento):
/// navegador do sistema, como no Client.
#[tauri::command]
async fn login_google_desktop(app: tauri::AppHandle, client_id: String) -> Result<GoogleLogin, String> {
    #[cfg(mobile)]
    {
        use tauri_plugin_google_auth::{GoogleAuthExt, SignInRequest};
        let _ = client_id;
        let pedido: SignInRequest = serde_json::from_value(serde_json::json!({
            "clientId": CLIENTE_WEB,
            "scopes": ["openid", "email", "profile"],
        }))
        .map_err(|e| e.to_string())?;
        let resposta = app.google_auth().sign_in(pedido).await.map_err(|e| e.to_string())?;
        let id_token = resposta.id_token.filter(|t| !t.is_empty()).ok_or("o Google não devolveu o token de identidade")?;
        Ok(GoogleLogin { id_token })
    }
    #[cfg(not(mobile))]
    {
        Ok(GoogleLogin { id_token: oauth::login(&app, &client_id).await? })
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_google_auth::init())
        .manage(mister::Sessao::default())
        .invoke_handler(tauri::generate_handler![
            login_google_desktop,
            mister::definir_token_autenticacao,
            mister::limpar_token_autenticacao,
            mister::mister_biblioteca,
            mister::mister_usuarios,
            mister::mister_conectar,
            mister::mister_enviar_token,
            mister::mister_jogos_do_jogador,
            mister::mister_iniciar,
            mister::mister_parar,
        ])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar o ShmupX Lite");
}
