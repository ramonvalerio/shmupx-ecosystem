//! Conectar o MiSTer FPGA (ADR 008): o backend gera um token ligado à conta logada e manda para
//! o e-mail dela. O token não passa pelo Client: a resposta só diz para onde foi enviado.

use reqwest::Method;
use serde_json::Value;
use tauri::State;

use super::SessionState;
use crate::net::backend::chamar_json;

/// `{ "enviado_para": "ra***@gmail.com" }`.
#[tauri::command]
pub async fn mister_enviar_token(state: State<'_, SessionState>) -> Result<Value, String> {
    let token = state
        .snapshot()
        .token
        .ok_or_else(|| "sem sessão: entre com sua conta para conectar o MiSTer".to_string())?;
    chamar_json(&token, Method::POST, "/api/mister/tokens", None).await
}
