use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::ffmpeg::{self, RecordingConfig};
use super::run_timer::TEST_RUN_DURATION;
use super::state::{RunHandle, RunState};

#[derive(Debug, Clone, Serialize)]
struct RunExpiredPayload {
    jogo_id: String,
}

/// Inicia uma corrida: valida o FFmpeg, faz o spawn do processo de captura em segundo plano
/// e agenda o timer seguro (TASKS.md 2.x / 3.x). Retorna o PID do processo FFmpeg.
#[tauri::command]
pub async fn iniciar_corrida_safe(
    app: AppHandle,
    state: State<'_, RunState>,
    jogo_id: String,
) -> Result<u32, String> {
    if !ffmpeg::is_ffmpeg_available() {
        return Err("ffmpeg não encontrado no PATH do sistema".into());
    }

    let mut guard = state.0.lock().await;
    if guard.is_some() {
        return Err("já existe uma corrida em andamento".into());
    }

    let output_path = std::env::temp_dir().join(format!("shmupx_run_{jogo_id}.mp4"));
    let config = RecordingConfig {
        output_path,
        rtmp_url: None,
    };

    let child = ffmpeg::spawn_ffmpeg(&config).map_err(|e| e.to_string())?;
    let pid = child
        .id()
        .ok_or_else(|| "processo FFmpeg sem PID (já finalizado?)".to_string())?;

    *guard = Some(RunHandle {
        child,
        pid,
        jogo_id: jogo_id.clone(),
    });
    drop(guard);

    spawn_safe_timer(app, state.0.clone(), jogo_id, TEST_RUN_DURATION);

    Ok(pid)
}

/// Encerra a corrida atual de forma graciosa (comando manual do jogador/UI).
#[tauri::command]
pub async fn finalizar_corrida_manual(state: State<'_, RunState>) -> Result<(), String> {
    let mut guard = state.0.lock().await;
    match guard.take() {
        Some(mut handle) => ffmpeg::graceful_stop(&mut handle.child)
            .await
            .map_err(|e| e.to_string()),
        None => Err("nenhuma corrida em andamento".into()),
    }
}

/// Timer seguro em segundo plano: se o tempo expirar antes de uma finalização manual,
/// mata o processo do FFmpeg imediatamente e notifica o frontend via evento `run://expired`
/// (ver docs/specs/002-economia-fichas-e-ranking.md, seção 2).
fn spawn_safe_timer(
    app: AppHandle,
    run_state: std::sync::Arc<tokio::sync::Mutex<Option<RunHandle>>>,
    jogo_id: String,
    limit: std::time::Duration,
) {
    tokio::spawn(async move {
        tokio::time::sleep(limit).await;

        let mut guard = run_state.lock().await;
        if let Some(mut handle) = guard.take() {
            if handle.jogo_id == jogo_id {
                let _ = handle.child.kill().await;
                let _ = app.emit("run://expired", RunExpiredPayload { jogo_id });
            } else {
                // Outra corrida assumiu o estado nesse intervalo; devolve o handle.
                *guard = Some(handle);
            }
        }
    });
}
