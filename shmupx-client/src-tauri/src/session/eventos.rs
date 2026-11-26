//! Eventos no Client: o carrossel da sala e a inscrição paga em fichas, em nome do jogador logado.
//! Quem decide preço, período e inscrição é o backend; aqui o JSON só passa.

use reqwest::Method;
use serde_json::Value;
use tauri::State;

use super::SessionState;
use crate::net::backend::chamar_json;

fn token(state: &State<'_, SessionState>) -> Result<String, String> {
    state.snapshot().token.ok_or_else(|| "sem sessão: entre com sua conta para ver os eventos".to_string())
}

/// Ids vêm do próprio backend; ainda assim, nada fora deste formato vira caminho de URL.
fn id_seguro(id: &str) -> Result<&str, String> {
    let ok = !id.is_empty() && id.len() <= 128 && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'));
    if ok { Ok(id) } else { Err("id de evento inválido".into()) }
}

/// Eventos publicados que ainda não terminaram, com a inscrição do jogador.
#[tauri::command]
pub async fn eventos_listar(state: State<'_, SessionState>) -> Result<Value, String> {
    chamar_json(&token(&state)?, Method::GET, "/api/events", None).await
}

/// Inscreve o jogador no evento, pagando as fichas da inscrição.
#[tauri::command]
pub async fn evento_participar(state: State<'_, SessionState>, id: String) -> Result<Value, String> {
    let id = id_seguro(&id)?;
    chamar_json(&token(&state)?, Method::POST, &format!("/api/events/{id}/join"), None).await
}

/// Página do evento (cadastrada no Lab e validada pelo backend): só `https://`, sem espaços nem aspas.
fn pagina_valida(url: &str) -> bool {
    url.len() > "https://".len() && url.len() <= 300 && url.starts_with("https://") && !url.chars().any(|c| c.is_whitespace() || matches!(c, '"' | '<' | '>' | '\''))
}

/// Abre a página do evento no navegador.
#[tauri::command]
pub fn evento_abrir_pagina(url: String) -> Result<(), String> {
    if !pagina_valida(&url) {
        return Err("link inválido".into());
    }
    crate::auth::oauth::abrir_navegador(&url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_paginas_https() {
        assert!(pagina_valida("https://www.shmupx.com/eventos/copa"));
        assert!(!pagina_valida("http://x.com") && !pagina_valida("file:///c:/x") && !pagina_valida("https://a b") && !pagina_valida("https://"));
    }

    #[test]
    fn ids_de_evento_nao_viram_caminhos() {
        assert!(id_seguro("copa-vertical").is_ok());
        assert!(id_seguro("../me").is_err());
        assert!(id_seguro("").is_err());
    }
}
