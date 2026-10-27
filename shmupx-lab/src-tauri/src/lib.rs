pub mod ai;
pub mod auth;
pub mod net;
pub mod validacao;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .manage(validacao::commands::ValidacaoState::default())
    .invoke_handler(tauri::generate_handler![
      ai::commands::sugerir_jogo,
      auth::commands::login_google_desktop,
      validacao::commands::validacao_ambiente,
      validacao::commands::validacao_listar,
      validacao::commands::validacao_adicionar_youtube,
      validacao::commands::validacao_adicionar_arquivos,
      validacao::commands::validacao_definir_gabarito,
      validacao::commands::validacao_remover,
      validacao::commands::validacao_rodar
    ])
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
