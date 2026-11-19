//! Sininho do Client: avisos de versão (e futuros avisos) vindos do backend, em nome do jogador
//! logado. O conteúdo passa como JSON, sem regra aqui: quem decide o que é lido, o que está
//! liberado e o que é obrigatório é o servidor.

use reqwest::Method;
use serde_json::{Value, json};
use tauri::State;

use super::SessionState;
use crate::net::backend::chamar_json;

fn token(state: &State<'_, SessionState>) -> Result<String, String> {
    state
        .snapshot()
        .token
        .ok_or_else(|| "sem sessão: entre com sua conta para ver as notificações".to_string())
}

/// Ids vêm do próprio backend; ainda assim, nada fora deste formato vira caminho de URL.
fn id_seguro(id: &str) -> Result<&str, String> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok {
        Ok(id)
    } else {
        Err("id de notificação inválido".into())
    }
}

/// Lista com o contador de não lidas (`{ unread, items }`).
#[tauri::command]
pub async fn notificacoes_listar(state: State<'_, SessionState>) -> Result<Value, String> {
    chamar_json(&token(&state)?, Method::GET, "/api/notifications", None).await
}

/// Conteúdo completo; o backend marca como lida ao abrir.
#[tauri::command]
pub async fn notificacoes_obter(
    state: State<'_, SessionState>,
    id: String,
) -> Result<Value, String> {
    let id = id_seguro(&id)?;
    chamar_json(
        &token(&state)?,
        Method::GET,
        &format!("/api/notifications/{id}"),
        None,
    )
    .await
}

#[tauri::command]
pub async fn notificacoes_marcar_todas(state: State<'_, SessionState>) -> Result<Value, String> {
    chamar_json(
        &token(&state)?,
        Method::POST,
        "/api/notifications/read-all",
        None,
    )
    .await
}

/// Liga/desliga os avisos por e-mail.
#[tauri::command]
pub async fn notificacoes_preferencia_email(
    state: State<'_, SessionState>,
    ligada: bool,
) -> Result<Value, String> {
    chamar_json(
        &token(&state)?,
        Method::PUT,
        "/api/me/notification-preferences",
        Some(json!({ "email_updates": ligada })),
    )
    .await
}

/// Situação desta versão do Client: mais nova liberada, próxima agendada e se precisa atualizar.
#[tauri::command]
pub async fn versao_situacao(state: State<'_, SessionState>) -> Result<Value, String> {
    chamar_json(&token(&state)?, Method::GET, "/api/client/version", None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_de_notificacao_nao_vira_caminho() {
        assert!(id_seguro("release-0.2.0").is_ok());
        assert!(id_seguro("../me").is_err());
        assert!(id_seguro("a/b").is_err());
        assert!(id_seguro("").is_err());
    }
}
