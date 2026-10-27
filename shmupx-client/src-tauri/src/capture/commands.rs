use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::analysis;
use crate::net::backend;
use crate::session::SessionState;

use super::ffmpeg::{self, RecordingConfig};
use super::state::{AnalysisQueue, RunHandle, RunState};

#[derive(Debug, Clone, Serialize)]
struct RunExpiredPayload {
    jogo_id: String,
    pid: u32,
}

/// Inicia uma corrida: começa a captura e confirma que o FFmpeg está gravando; só então debita
/// a ficha no backend (`POST /api/runs/start`) e usa a `max_duration_segundos` devolvida para o
/// timer seguro. Uma captura que não inicia não custa ficha.
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

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("não foi possível resolver a pasta de dados do app: {e}"))?;
    std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;

    // Cada run preserva seu vídeo mesmo se outra começar durante a análise. O nome é local (a
    // captura começa antes de o backend devolver o id da run).
    let carimbo = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    let output_path = data_dir.join(format!("shmupx_run_{carimbo}.mp4"));
    let config = RecordingConfig::new(output_path.clone());

    let mut child = ffmpeg::spawn_ffmpeg(&config).map_err(|e| format!("falha ao iniciar o FFmpeg: {e}"))?;
    if let Err(erro) = ffmpeg::confirmar_inicio(&mut child, &config).await {
        let _ = std::fs::remove_file(&output_path);
        return Err(erro.to_string());
    }
    let pid = child
        .id()
        .ok_or_else(|| "processo FFmpeg sem PID (já finalizado?)".to_string())?;

    // Débito de ficha + abertura da "ficha ativa" no backend com a captura já confirmada. Se o
    // backend recusar (sem saldo, jogo não publicado, token inválido), a captura é descartada.
    let inicio = match backend::iniciar_run(&token, &jogo_id).await {
        Ok(inicio) => inicio,
        Err(e) => {
            let _ = child.kill().await;
            let _ = std::fs::remove_file(&output_path);
            return Err(format!("backend recusou iniciar a run: {e}"));
        }
    };
    let run_id = inicio.run_id.clone();

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

/// Para a captura, avisa o backend do fim da gameplay e põe a análise na fila. Retorna logo:
/// o jogador pode iniciar outra run enquanto esta é analisada em segundo plano.
#[tauri::command]
pub async fn finalizar_corrida_manual(
    app: AppHandle,
    state: State<'_, RunState>,
) -> Result<(), String> {
    let mut guard = state
        .0
        .try_lock()
        .map_err(|_| "aguarde a parada da corrida anterior".to_string())?;
    let mut handle = guard
        .take()
        .ok_or("nenhuma corrida em andamento".to_string())?;
    if let Err(error) = ffmpeg::graceful_stop(&mut handle.child).await {
        emit_analysis(&app, &handle.jogo_id, handle.pid, "failed", Some(format!("falha ao encerrar captura: {error}")));
        return Err(error.to_string());
    }
    // Com a captura fechada, a próxima run já pode começar.
    drop(guard);
    encerrar_e_enfileirar(&app, handle).await;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct RunAnalysisPayload {
    jogo_id: String,
    pid: u32,
    status: String,
    message: Option<String>,
}

fn emit_analysis(app: &AppHandle, jogo_id: &str, pid: u32, status: &str, message: Option<String>) {
    let _ = app.emit(
        "run://analysis",
        RunAnalysisPayload { jogo_id: jogo_id.to_string(), pid, status: status.into(), message },
    );
}

/// O que a análise precisa depois que a captura terminou (sem o processo do FFmpeg).
struct AnaliseJob {
    pid: u32,
    jogo_id: String,
    run_id: String,
    token: String,
    video_path: PathBuf,
}

/// O ID token do Firebase vale ~1 h: uma análise longa na fila envia com o token atual da sessão.
fn token_atual(app: &AppHandle, guardado: &str) -> String {
    app.state::<SessionState>().snapshot().token.unwrap_or_else(|| guardado.to_string())
}

/// Registra no servidor o fim da gameplay e agenda a análise. Se o servidor disser que a
/// gameplay passou do tempo da ficha, não há o que analisar; uma falha de rede não impede a
/// análise (o envio ainda cabe na janela da gameplay).
async fn encerrar_e_enfileirar(app: &AppHandle, handle: RunHandle) {
    let job = AnaliseJob {
        pid: handle.pid,
        jogo_id: handle.jogo_id,
        run_id: handle.run_id,
        token: handle.token,
        video_path: handle.video_path,
    };
    match backend::encerrar_run(&token_atual(app, &job.token), &job.run_id).await {
        Ok(()) => {}
        Err(backend::FalhaEncerramento::Recusado(erro)) => {
            log::warn!("capture: run {} recusada no encerramento: {erro}", job.run_id);
            emit_analysis(app, &job.jogo_id, job.pid, "failed", Some(format!("vídeo preservado; a run foi encerrada pelo servidor: {erro}")));
            return;
        }
        Err(backend::FalhaEncerramento::Outro(erro)) => {
            log::warn!("capture: aviso de encerramento da run {} falhou: {erro}", job.run_id);
        }
    }
    emit_analysis(app, &job.jogo_id, job.pid, "analyzing", None);
    let app = app.clone();
    tauri::async_runtime::spawn(async move { analisar_e_enviar(&app, job).await });
}

/// Espera a vez na fila, analisa o vídeo e envia o resultado. Sem prazo para a análise: o limite
/// de tempo é da gameplay, já registrado no encerramento.
async fn analisar_e_enviar(app: &AppHandle, job: AnaliseJob) {
    let fila = app.state::<AnalysisQueue>().0.clone();
    let _vez = fila.lock().await;
    let result = async {
        let result = analysis::analisar_run(app, &job.video_path, &job.jogo_id).await?;
        backend::submeter_run(
            &token_atual(app, &job.token),
            &job.run_id,
            &job.jogo_id,
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
        Ok(()) => emit_analysis(app, &job.jogo_id, job.pid, "submitted", None),
        Err(error) => {
            log::warn!("capture: run {} não enviada: {error}", job.run_id);
            emit_analysis(app, &job.jogo_id, job.pid, "failed", Some(error));
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
            drop(guard);
            if let Err(error) = handle.child.kill().await {
                emit_analysis(&app, &handle.jogo_id, handle.pid, "failed", Some(format!("falha ao cortar captura: {error}")));
                return;
            }
            let _ = app.emit("run://expired", RunExpiredPayload { jogo_id, pid: handle.pid });
            encerrar_e_enfileirar(&app, handle).await;
        }
    });
}
