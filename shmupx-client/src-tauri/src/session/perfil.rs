//! Perfil público (nick e avatar) e páginas do site: o Client chama o backend em nome do jogador
//! (o ID token fica no Rust) e abre no navegador só páginas do próprio site.

use reqwest::Method;
use serde_json::Value;
use tauri::State;

use super::SessionState;
use crate::net::backend::chamar_json;

fn token(state: &SessionState) -> Result<String, String> {
    state.snapshot().token.ok_or_else(|| "sem sessão: entre com sua conta".to_string())
}

/// Nick: o mesmo formato que o backend aceita (o caminho não leva nada além disso).
fn nick_seguro(nick: &str) -> bool {
    !nick.is_empty() && nick.len() <= 20 && nick.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Perfil público do jogador; o nome entra só como sugestão do nick padrão (primeiro acesso).
#[tauri::command]
pub async fn perfil_obter(state: State<'_, SessionState>, nome: Option<String>) -> Result<Value, String> {
    let consulta = nome
        .map(|n| format!("?display_name={}", url::form_urlencoded::byte_serialize(n.as_bytes()).collect::<String>()))
        .unwrap_or_default();
    chamar_json(&token(&state)?, Method::GET, &format!("/api/me/profile{consulta}"), None).await
}

/// `{ valid, available }` do nick.
#[tauri::command]
pub async fn perfil_nick_disponivel(state: State<'_, SessionState>, nick: String) -> Result<Value, String> {
    let nick = nick.trim().to_lowercase();
    if !nick_seguro(&nick) {
        return Ok(serde_json::json!({ "valid": false, "available": false }));
    }
    chamar_json(&token(&state)?, Method::GET, &format!("/api/handles/{nick}"), None).await
}

/// A troca livre do nick (uma vez).
#[tauri::command]
pub async fn perfil_trocar_nick(state: State<'_, SessionState>, nick: String) -> Result<Value, String> {
    chamar_json(&token(&state)?, Method::PUT, "/api/me/handle", Some(serde_json::json!({ "handle": nick }))).await
}

/// Pedido de troca ao suporte (depois da troca livre).
#[tauri::command]
pub async fn perfil_pedir_nick(state: State<'_, SessionState>, nick: String) -> Result<Value, String> {
    chamar_json(&token(&state)?, Method::POST, "/api/me/handle/request", Some(serde_json::json!({ "handle": nick }))).await
}

/// Avatar recortado (data URL) ou `None` para voltar ao padrão.
#[tauri::command]
pub async fn perfil_avatar(state: State<'_, SessionState>, avatar: Option<String>) -> Result<Value, String> {
    chamar_json(&token(&state)?, Method::PUT, "/api/me/avatar", Some(serde_json::json!({ "avatar": avatar }))).await
}

/// Abre uma página do site no navegador (ex.: a de assinatura).
#[tauri::command]
pub fn site_abrir(caminho: String) -> Result<(), String> {
    let seguro = caminho.starts_with('/') && caminho.chars().all(|c| c.is_ascii_alphanumeric() || "/-_?=&".contains(c));
    if !seguro {
        return Err("página inválida".into());
    }
    crate::auth::oauth::abrir_navegador(&format!("https://www.shmupx.com{caminho}"))
}
