use std::io;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::detector;

/// Janela de tempo em que o monitor de lançamento aguarda o processo do jogo aparecer.
const LAUNCH_MONITOR_TIMEOUT: Duration = Duration::from_secs(60);
/// Com a Steam fechada, ela abre primeiro (login, atualização) e só depois inicia o jogo.
const LAUNCH_MONITOR_TIMEOUT_STEAM_FECHADA: Duration = Duration::from_secs(180);
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

    // Com a Steam fechada, o `steam://run` a abre e ela inicia o jogo em seguida (como no Client
    // antigo): só é preciso esperar mais pelo processo do jogo.
    let steam_aberta = detector::is_steam_running();

    launch_steam_uri(steam_app_id)
        .map_err(|e| format!("falha ao disparar steam://run/{steam_app_id}: {e}"))?;

    let limite = if steam_aberta { LAUNCH_MONITOR_TIMEOUT } else { LAUNCH_MONITOR_TIMEOUT_STEAM_FECHADA };
    spawn_launch_monitor(app, steam_app_id, limite);

    Ok(())
}

/// Dispara o protocolo nativo `steam://run/<app_id>` usando o abridor de URIs do sistema
/// operacional (equivalente ao "Executar" do Windows / `open` do macOS / `xdg-open` do Linux).
pub(crate) fn launch_steam_uri(app_id: u32) -> io::Result<()> {
    let uri = format!("steam://run/{app_id}");

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &uri])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW: sem piscar um console
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
fn spawn_launch_monitor(app: AppHandle, app_id: u32, limite: Duration) {
    tokio::spawn(async move {
        let baseline = detector::snapshot_pids();
        let pasta = tauri::async_runtime::spawn_blocking(move || super::biblioteca::pasta_do_jogo(app_id)).await.ok().flatten();
        let mut elapsed = Duration::ZERO;

        loop {
            tokio::time::sleep(LAUNCH_MONITOR_POLL_INTERVAL).await;
            elapsed += LAUNCH_MONITOR_POLL_INTERVAL;

            if let Some((pid, name)) = detector::find_new_process(&baseline, pasta.as_deref()) {
                let _ = app.emit(
                    "steam://game-launched",
                    GameLaunchedPayload { app_id, pid, name },
                );
                return;
            }

            if elapsed >= limite {
                let _ = app.emit(
                    "steam://game-launch-timeout",
                    GameLaunchTimeoutPayload { app_id },
                );
                return;
            }
        }
    });
}

#[derive(Debug, Clone, Serialize)]
struct EmuladorAbertoPayload {
    game_id: String,
    pid: u32,
    rom: String,
}

/// Inicializa um jogo de emulador no FBNeo do Fightcade. `roms`: os ROMs que correspondem ao jogo
/// (a interface cruza com o catálogo); `pasta`: a do Fightcade, se o jogador escolheu uma. Avisa
/// `emulator://game-launched` com o processo do emulador, que a gravação vigia como na Steam.
#[tauri::command]
pub async fn inicializar_jogo_emulador(app: AppHandle, jogo_id: String, roms: Vec<String>, pasta: Option<String>) -> Result<(), String> {
    let (pid, rom) = tauri::async_runtime::spawn_blocking(move || super::emulador::abrir(pasta.as_deref(), &roms))
        .await
        .map_err(|e| e.to_string())??;
    let _ = app.emit("emulator://game-launched", EmuladorAbertoPayload { game_id: jogo_id, pid, rom });
    Ok(())
}

/// Jogos instalados na Steam deste PC (`libraryfolders.vdf` + `appmanifest_*.acf`).
#[tauri::command]
pub async fn steam_biblioteca() -> super::biblioteca::Biblioteca {
    tauri::async_runtime::spawn_blocking(super::biblioteca::ler)
        .await
        .unwrap_or(super::biblioteca::Biblioteca { found: false, games: vec![] })
}

/// ROMs do emulador (Fightcade/FBNeo) deste PC. `pasta`: a do Fightcade, se o jogador escolheu uma.
#[tauri::command]
pub async fn emulador_biblioteca(pasta: Option<String>) -> super::emulador::BibliotecaEmulador {
    tauri::async_runtime::spawn_blocking(move || super::emulador::ler(pasta.as_deref()))
        .await
        .unwrap_or(super::emulador::BibliotecaEmulador { found: false, root: None, roms: vec![] })
}

/// Jogadores já configurados no `shmupx.ini` do MiSTer.
#[tauri::command]
pub async fn mister_usuarios(host: String, senha: Option<String>) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || super::mister::usuarios(&host, senha.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

/// Conecta o MiSTer à conta: pede um token novo ao backend e o grava direto no `shmupx.ini` do
/// MiSTer para o jogador `username`. O token não volta para a interface.
#[tauri::command]
pub async fn mister_conectar(
    sessao: tauri::State<'_, crate::session::SessionState>,
    host: String,
    senha: Option<String>,
    username: String,
) -> Result<(), String> {
    let login = sessao.snapshot().token.ok_or_else(|| "entre na sua conta para conectar o MiSTer".to_string())?;
    let resposta = crate::net::backend::chamar_json(&login, reqwest::Method::POST, "/api/mister/tokens/direct", None).await?;
    let token = resposta["token"].as_str().filter(|t| t.starts_with("sxm_")).ok_or("o servidor não devolveu o token do MiSTer")?.to_string();
    let api = crate::net::backend::BACKEND_BASE_URL;
    tauri::async_runtime::spawn_blocking(move || super::mister::gravar_token(&host, senha.as_deref(), &username, &token, api))
        .await
        .map_err(|e| e.to_string())?
}

/// Jogos do MiSTer FPGA na rede (FTP, usuário root). `senha`: a do MiSTer, se não for a padrão.
#[tauri::command]
pub async fn mister_biblioteca(host: String, senha: Option<String>) -> Result<super::mister::BibliotecaMister, String> {
    tauri::async_runtime::spawn_blocking(move || super::mister::ler(&host, senha.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}
