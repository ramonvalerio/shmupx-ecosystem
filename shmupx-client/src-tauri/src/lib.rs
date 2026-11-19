//! ShmupX Client — backend Rust (Tauri v2).
//!
//! Domínios (ver docs/adr/001-arquitetura-monorepo-ddd.md):
//! - `steam`   : detecção de processos/jogos Steam em execução.
//! - `capture` : controle do FFmpeg e timer seguro da run.
//! - `input`   : hotkeys globais de joystick/controle em segundo plano.

pub mod analysis;
pub mod auth;
pub mod capture;
pub mod input;
pub mod net;
pub mod session;
pub mod steam;

use tauri::Manager;

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(capture::RunState::default())
        .manage(capture::AnalysisQueue::default())
        .manage(capture::RecordingState::default())
        .manage(session::SessionState::default())
        .invoke_handler(tauri::generate_handler![
            app_version,
            capture::commands::iniciar_corrida_safe,
            capture::commands::finalizar_corrida_manual,
            steam::commands::inicializar_jogo_steam,
            session::commands::definir_token_autenticacao,
            session::commands::limpar_token_autenticacao,
            session::commands::definir_sala_ativa,
            session::notificacoes::notificacoes_listar,
            session::notificacoes::notificacoes_obter,
            session::notificacoes::notificacoes_marcar_todas,
            session::notificacoes::notificacoes_preferencia_email,
            session::notificacoes::versao_situacao,
            session::mister::mister_enviar_token,
            auth::commands::login_google_desktop,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            capture::ffmpeg::usar_empacotado(app.path().resource_dir().ok());
            input::spawn_listener(app.handle().clone());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
