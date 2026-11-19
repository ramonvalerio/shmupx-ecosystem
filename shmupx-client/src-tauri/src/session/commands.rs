use tauri::State;

use super::SessionState;

/// Chamado pelo frontend com o ID token do Firebase a cada login e renovação (~1 h). Todas as
/// chamadas HTTP que o Rust faz em nome do jogador (início/submissão de run, upload de voz) usam
/// este token.
#[tauri::command]
pub fn definir_token_autenticacao(state: State<'_, SessionState>, token: String) {
    state
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .token = Some(token);
}

/// Chamado pelo frontend quando o jogador entra numa sala. Necessário para o upload de voz do
/// RobotX (`net::robotx::enviar_audio_ptt`), que precisa de um `sala_id` para a Edge Function
/// resolver o `jogo_id` no servidor (ADR 003).
#[tauri::command]
pub fn definir_sala_ativa(state: State<'_, SessionState>, sala_id: String) {
    state
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .sala_id = Some(sala_id);
}

/// Chamado pelo frontend no logout: o Rust para de agir em nome do jogador.
#[tauri::command]
pub fn limpar_token_autenticacao(state: State<'_, SessionState>) {
    state
        .0
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .token = None;
}
