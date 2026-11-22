//! Comandos da live e da conexão do YouTube (Configurações › Integrações).

use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use super::{conduzir, Estado, LiveState};
use crate::auth::oauth;
use crate::capture::RunState;
use crate::net::backend::chamar_json;
use crate::session::SessionState;

fn token(sessao: &SessionState) -> Result<String, String> {
    sessao.snapshot().token.ok_or_else(|| "Entre na sua conta para usar o YouTube.".to_string())
}

/// Canal conectado? `{ connected, channel_title }`.
#[tauri::command]
pub async fn youtube_conexao(sessao: State<'_, SessionState>) -> Result<Value, String> {
    chamar_json(&token(&sessao)?, reqwest::Method::GET, "/api/youtube/connection", None).await
}

/// Abre o consentimento do Google para o YouTube (transmitir no canal) e guarda a conexão no
/// servidor. O app nunca recebe o token do Google.
#[tauri::command]
pub async fn youtube_conectar(app: AppHandle, sessao: State<'_, SessionState>) -> Result<Value, String> {
    let token = token(&sessao)?;
    let config = chamar_json(&token, reqwest::Method::GET, "/api/youtube/config", None).await?;
    let client_id = config["client_id"].as_str().ok_or("YouTube não configurado no servidor")?;
    let escopo = config["scope"].as_str().ok_or("YouTube não configurado no servidor")?;
    // `offline` + `consent`: o Google só entrega o acesso contínuo (refresh) com consentimento novo.
    let codigo = oauth::consentimento(
        client_id,
        escopo,
        &[("access_type", "offline"), ("prompt", "consent select_account"), ("include_granted_scopes", "true")],
    )
    .await;
    oauth::trazer_para_frente(&app);
    let codigo = codigo?;
    chamar_json(
        &token,
        reqwest::Method::POST,
        "/api/youtube/connect",
        Some(serde_json::json!({ "code": codigo.code, "code_verifier": codigo.verifier, "redirect_uri": codigo.redirect_uri })),
    )
    .await
}

#[tauri::command]
pub async fn youtube_desconectar(sessao: State<'_, SessionState>, live: State<'_, LiveState>) -> Result<Value, String> {
    if live.ativa() {
        return Err("Encerre a live antes de desconectar o YouTube.".into());
    }
    chamar_json(&token(&sessao)?, reqwest::Method::DELETE, "/api/youtube/connection", None).await
}

/// Quantos assistem agora à live do próprio jogador (o contador da janela da live na sala).
#[tauri::command]
pub async fn live_espectadores(sessao: State<'_, SessionState>, broadcast_id: String) -> Result<u64, String> {
    if broadcast_id.is_empty() || broadcast_id.len() > 64 || !broadcast_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("id da live inválido".into());
    }
    let resposta = chamar_json(&token(&sessao)?, reqwest::Method::GET, &format!("/api/live/broadcasts/{broadcast_id}/viewers"), None).await?;
    Ok(resposta["viewers"].as_u64().unwrap_or(0))
}

#[tauri::command]
pub fn live_estado(live: State<'_, LiveState>) -> Estado {
    live.snapshot()
}

/// START LIVE: começa em segundo plano; o andamento chega pelo evento `live://estado`.
#[tauri::command]
pub async fn live_iniciar(
    app: AppHandle,
    sessao: State<'_, SessionState>,
    live: State<'_, LiveState>,
    run: State<'_, RunState>,
    jogo_id: String,
    titulo: String,
    publica: bool,
    // Jogo da sala: app_id da Steam (0/ausente = emulador). A live captura só a janela dele.
    steam_app_id: Option<u32>,
) -> Result<(), String> {
    let token = token(&sessao)?;
    if live.ativa() {
        return Err("Já existe uma live em andamento.".into());
    }
    if run.0.lock().await.is_some() {
        return Err("Termine a gravação da run antes de começar uma live.".into());
    }
    let log = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("pasta de dados indisponível: {e}"))?
        .join("live.ffmpeg.log");
    if let Some(pasta) = log.parent() {
        let _ = std::fs::create_dir_all(pasta);
    }
    let estado = live.inner().clone();
    tauri::async_runtime::spawn(conduzir(app, estado, token, jogo_id, titulo, publica, steam_app_id, log));
    Ok(())
}

/// END LIVE (também cancela um início em andamento).
#[tauri::command]
pub fn live_encerrar(live: State<'_, LiveState>) {
    live.cancelar.notify_waiters();
}

/// Abre no navegador o link da live ou as páginas do YouTube para criar o canal e liberar as lives
/// (só esses endereços).
#[tauri::command]
pub fn live_abrir(url: String) -> Result<(), String> {
    const PERMITIDOS: [&str; 3] = ["https://www.youtube.com/watch?v=", "https://www.youtube.com/create_channel", "https://www.youtube.com/features"];
    if !PERMITIDOS.iter().any(|p| url.starts_with(p)) {
        return Err("link inválido".into());
    }
    oauth::abrir_navegador(&url)
}
