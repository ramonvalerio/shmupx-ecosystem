use std::io;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::detector;

/// Janela de tempo em que o monitor de lançamento aguarda o processo do jogo aparecer.
const LAUNCH_MONITOR_TIMEOUT: Duration = Duration::from_secs(60);
const LAUNCH_MONITOR_POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize)]
struct GameLaunchedPayload {
    app_id: u32,
    pid: u32,
    name: String,
}

#[derive(Debug, Clone, Serialize)]
struct GameLaunchTimeoutPayload {
    app_id: u32,
}

/// Inicializa um jogo via protocolo oficial da Steam (`steam://run/<app_id>`), garantindo que
/// overlay e conquistas funcionem normalmente (ver docs/adr/001, bounded context "Sessão de Jogo").
#[tauri::command]
pub async fn inicializar_jogo_steam(app: AppHandle, steam_app_id: u32) -> Result<(), String> {
    if steam_app_id == 0 {
        return Err("steam_app_id inválido (esperado um ID positivo)".into());
    }

    if !detector::is_steam_running() {
        return Err(
            "Steam não está em execução. Abra o cliente da Steam antes de iniciar o jogo.".into(),
        );
    }

    launch_steam_uri(steam_app_id)
        .map_err(|e| format!("falha ao disparar steam://run/{steam_app_id}: {e}"))?;

    spawn_launch_monitor(app, steam_app_id);

    Ok(())
}

/// Dispara o protocolo nativo `steam://run/<app_id>` usando o abridor de URIs do sistema
/// operacional (equivalente ao "Executar" do Windows / `open` do macOS / `xdg-open` do Linux).
fn launch_steam_uri(app_id: u32) -> io::Result<()> {
    let uri = format!("steam://run/{app_id}");

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &uri])
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(&uri).spawn()?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(&uri).spawn()?;
    }

    Ok(())
}

/// Monitoramento em background (stub/lógica inicial — TASKS.md 1.5): após disparar o jogo,
/// faz polling por um novo processo no sistema. Quando a detecção precisa (manifesto `.acf`,
/// TASKS.md 1.2–1.4) existir, este é o ponto que vai disparar a gravação do FFmpeg
/// automaticamente ao confirmar que o jogo abriu.
fn spawn_launch_monitor(app: AppHandle, app_id: u32) {
    tokio::spawn(async move {
        let baseline = detector::snapshot_pids();
        let mut elapsed = Duration::ZERO;

        loop {
            tokio::time::sleep(LAUNCH_MONITOR_POLL_INTERVAL).await;
            elapsed += LAUNCH_MONITOR_POLL_INTERVAL;

            if let Some((pid, name)) = detector::find_new_process(&baseline) {
                let _ = app.emit(
                    "steam://game-launched",
                    GameLaunchedPayload { app_id, pid, name },
                );
                return;
            }

            if elapsed >= LAUNCH_MONITOR_TIMEOUT {
                let _ = app.emit(
                    "steam://game-launch-timeout",
                    GameLaunchTimeoutPayload { app_id },
                );
                return;
            }
        }
    });
}
