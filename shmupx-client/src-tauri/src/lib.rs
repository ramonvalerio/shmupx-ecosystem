//! ShmupX Client — backend Rust (Tauri v2).
//!
//! Domínios (ver docs/adr/001-arquitetura-monorepo-ddd.md):
//! - `steam`   : detecção de processos/jogos Steam em execução.
//! - `capture` : controle do FFmpeg e timer seguro da run.
//! - `input`   : hotkeys globais de joystick/controle em segundo plano.

pub mod capture;
pub mod input;
pub mod steam;

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_version])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
