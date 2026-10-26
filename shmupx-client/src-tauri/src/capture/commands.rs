use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::analysis;
use crate::net::backend;
use crate::session::SessionState;

use super::ffmpeg::{self, RecordingConfig};
use super::state::{RunHandle, RunState};

#[derive(Debug, Clone, Serialize)]
struct RunExpiredPayload {
    jogo_id: String,
    pid: u32,
}

/// Inicia uma corrida: valida o FFmpeg, debita a ficha no backend (`POST /api/runs/start`),
/// usa a `max_duration_segundos` que o backend devolveu — não um valor fixo local — para o
/// timer seguro, e só então faz o spawn do processo de captura.
#[tauri::command]
pub async fn iniciar_corrida_safe(
    app: AppHandle,
    state: State<'_, RunState>,
    session: State<'_, SessionState>,
    jogo_id: String,
) -> Result<u32, String> {
    if !ffmpeg::is_ffmpeg_available() {
        return Err("ffmpeg não encontrado no PATH do sistema".into());
    }

    let mut guard = state
        .0
        .try_lock()
        .map_err(|_| "aguarde o processamento da corrida anterior".to_string())?;
    if guard.is_some() {
        return Err("já existe uma corrida em andamento".into());
    }

    let token = session.snapshot().token.ok_or_else(|| {
        "sem sessão: entre com sua conta Google antes de iniciar uma run".to_string()
    })?;

    // Débito de ficha + abertura da "ficha ativa" no backend ANTES de qualquer gravação local —
    // se o backend recusar (sem saldo, jogo/jogador inexistente, token inválido/expirado), a
    // run nem chega a começar a gravar.
    let inicio = backend::iniciar_run(&token, &jogo_id)
        .await
        .map_err(|e| format!("backend recusou iniciar a run: {e}"))?;

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("não foi possível resolver a pasta de dados do app: {e}"))?;
    std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;

    // Cada run preserva seu vídeo mesmo se outra começar durante a análise.
    let safe_id: String = inicio
        .run_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    if safe_id.is_empty() {
        return Err("backend retornou ID de run inválido".into());
    }
    let output_path = data_dir.join(format!("shmupx_run_{safe_id}.mp4"));
    let run_id = inicio.run_id.clone();
    let config = RecordingConfig::new(output_path.clone());

    let child = ffmpeg::spawn_ffmpeg(&config).map_err(|e| e.to_string())?;
    let pid = child
        .id()
        .ok_or_else(|| "processo FFmpeg sem PID (já finalizado?)".to_string())?;

    *guard = Some(RunHandle {
        child,
        pid,
        jogo_id: jogo_id.clone(),
        run_id: inicio.run_id,
        token: token.clone(),
        video_path: output_path,
    });
    drop(guard);

    // `max_duration_segundos` negativo é impossível vindo de um backend correto, mas `as u64`
    // em um `i32` negativo faria um número gigante — `.max(0)` evita essa armadilha.
    let limite = std::time::Duration::from_secs(inicio.max_duration_segundos.max(0) as u64);
    spawn_safe_timer(app, state.0.clone(), jogo_id, run_id, limite);

    Ok(pid)
}

/// Para a captura e processa a gravação. Eventos comunicam análise/envio/falha à UI.
#[tauri::command]
pub async fn finalizar_corrida_manual(
    app: AppHandle,
    state: State<'_, RunState>,
) -> Result<(), String> {
    let mut guard = state
        .0
        .try_lock()
        .map_err(|_| "aguarde o processamento da corrida anterior".to_string())?;
    let mut handle = guard
        .take()
        .ok_or("nenhuma corrida em andamento".to_string())?;
    // Mantém exclusão até o envio: o backend ainda associa submissões à última run ativa.
    if let Err(error) = ffmpeg::graceful_stop(&mut handle.child).await {
        emit_analysis(
            &app,
            &handle,
            "failed",
            Some(format!("falha ao encerrar captura: {error}")),
        );
        return Err(error.to_string());
    }
    process_run(&app, &handle).await;
    drop(guard);
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct RunAnalysisPayload {
    jogo_id: String,
    pid: u32,
    status: String,
    message: Option<String>,
}

fn emit_analysis(app: &AppHandle, handle: &RunHandle, status: &str, message: Option<String>) {
    let _ = app.emit(
        "run://analysis",
        RunAnalysisPayload {
            jogo_id: handle.jogo_id.clone(),
            pid: handle.pid,
            status: status.into(),
            message,
        },
    );
}

async fn process_run(app: &AppHandle, handle: &RunHandle) {
    emit_analysis(app, handle, "analyzing", None);
    let result = async {
        let result = analysis::analisar_run(app, &handle.video_path, &handle.jogo_id).await?;
        backend::submeter_run(
            &handle.token,
            &handle.jogo_id,
            result.score_bruto,
            result.vidas_perdidas,
            result.continues_usados,
        )
        .await
        .map_err(|error| format!("vídeo preservado; falha ao enviar resultado: {error}"))?;
        Ok::<(), String>(())
    }
    .await;
    match result {
        Ok(()) => emit_analysis(app, handle, "submitted", None),
        Err(error) => {
            log::warn!("capture: run {} não enviada: {error}", handle.run_id);
            emit_analysis(app, handle, "failed", Some(error));
        }
    }
}

/// O timer pertence a uma run específica, não apenas ao jogo (pode haver outra run depois).
fn spawn_safe_timer(
    app: AppHandle,
    run_state: std::sync::Arc<tokio::sync::Mutex<Option<RunHandle>>>,
    jogo_id: String,
    run_id: String,
    limit: std::time::Duration,
) {
    tokio::spawn(async move {
        tokio::time::sleep(limit).await;
        let mut guard = run_state.lock().await;
        if guard.as_ref().is_none_or(|handle| handle.run_id != run_id) {
            return;
        }
        if let Some(mut handle) = guard.take() {
            if let Err(error) = handle.child.kill().await {
                emit_analysis(
                    &app,
                    &handle,
                    "failed",
                    Some(format!("falha ao cortar captura: {error}")),
                );
                return;
            }
            let _ = app.emit(
                "run://expired",
                RunExpiredPayload {
                    jogo_id,
                    pid: handle.pid,
                },
            );
            process_run(&app, &handle).await;
            drop(guard);
        }
    });
}
