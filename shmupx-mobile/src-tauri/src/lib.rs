//! ShmupX Lite: o app do celular só entra nas salas, conversa e assiste às lives. Tudo isso é
//! Firebase no lado da interface; o Rust só conduz o login do Google (que precisa do navegador do
//! sistema e da troca do código no backend).

mod oauth;

use serde::Serialize;

#[derive(Serialize)]
struct GoogleLogin {
    id_token: String,
}

/// Mesmo nome do comando do Client (a interface reaproveitada chama este).
#[tauri::command]
async fn login_google_desktop(app: tauri::AppHandle, client_id: String) -> Result<GoogleLogin, String> {
    Ok(GoogleLogin { id_token: oauth::login(&app, &client_id).await? })
}

/// O Client guarda o ID token no Rust para chamar o backend; o Lite não chama o backend depois do
/// login, então só aceita (a interface reaproveitada chama sempre).
#[tauri::command]
fn definir_token_autenticacao(_token: String) {}

#[tauri::command]
fn limpar_token_autenticacao() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![login_google_desktop, definir_token_autenticacao, limpar_token_autenticacao])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar o ShmupX Lite");
}
