//! ShmupX Client — backend Rust (Tauri v2).
//!
//! Domínios (ver docs/adr/001-arquitetura-monorepo-ddd.md):
//! - `steam`   : detecção de processos/jogos Steam em execução.
//! - `capture` : controle do FFmpeg e timer seguro da run.
//! - `input`   : hotkeys globais de joystick/controle em segundo plano.

pub mod analysis;
pub mod auth;
pub mod capture;
pub mod chat;
pub mod input;
pub mod interface;
pub mod live;
pub mod net;
pub mod portatil;
pub mod session;
pub mod steam;

use tauri::Manager;

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn run() {
    // A interface baixada (sem instalador) entra no lugar da embutida, na mesma origem.
    let mut context = tauri::generate_context!();
    interface::instalar(&mut context);
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(capture::RunState::default())
        .manage(capture::AnalysisQueue::default())
        .manage(capture::RecordingState::default())
        .manage(session::SessionState::default())
        .manage(live::LiveState::default())
        .manage(steam::controles::CapturaState::default())
        .invoke_handler(tauri::generate_handler![
            app_version,
            interface::interface_aplicar,
            interface::interface_confirmar,
            interface::interface_versao,
            portatil::portatil_ativo,
            portatil::portatil_baixar,
            portatil::portatil_instalar,
            capture::commands::iniciar_corrida_safe,
            capture::commands::finalizar_corrida_manual,
            capture::commands::runs_pendentes,
            capture::commands::reanalisar_run,
            capture::commands::descartar_run_pendente,
            capture::ranking_local::ranking_local,
            capture::destino::gravacoes_destino,
            capture::destino::gravacoes_definir_destino,
            capture::destino::gravacoes_definir_gravar,
            capture::destino::gravacoes_abrir_pasta,
            steam::commands::inicializar_jogo_steam,
            steam::commands::inicializar_jogo_emulador,
            steam::dupla::dupla_preparar,
            steam::controles::motor_captura_iniciar,
            steam::controles::motor_captura_parar,
            steam::commands::steam_biblioteca,
            steam::commands::emulador_biblioteca,
            steam::commands::emulador_monitores,
            steam::commands::emulador_identificar_monitor,
            steam::commands::salvar_cartao_jogador,
            steam::commands::salvar_posicao_jogador,
            steam::commands::imagem_como_data_url,
            steam::commands::emulador_definir_pasta,
            steam::commands::emulador_baixar_roms,
            steam::commands::mister_biblioteca,
            steam::commands::mister_usuarios,
            steam::commands::mister_conectar,
            steam::commands::mister_jogos_do_jogador,
            steam::commands::mister_iniciar,
            steam::commands::mister_alcance,
            steam::commands::registrar_falha,
            steam::commands::mister_parar,
            steam::commands::mister_atualizar_agente,
            steam::commands::mister_sincronizar_menu,
            steam::trailer::steam_trailer,
            steam::commands::mister_procurar,
            steam::treino::treino_parar,
            session::commands::definir_token_autenticacao,
            session::commands::limpar_token_autenticacao,
            session::commands::definir_sala_ativa,
            session::notificacoes::notificacoes_listar,
            session::eventos::eventos_listar,
            session::eventos::evento_participar,
            session::eventos::evento_abrir_pagina,
            session::notificacoes::notificacoes_obter,
            session::notificacoes::notificacoes_marcar_todas,
            session::notificacoes::notificacoes_preferencia_email,
            session::notificacoes::versao_situacao,
            session::mister::mister_enviar_token,
            session::perfil::perfil_obter,
            session::perfil::perfil_nick_disponivel,
            session::perfil::perfil_trocar_nick,
            session::perfil::perfil_pedir_nick,
            session::perfil::perfil_avatar,
            session::perfil::site_abrir,
            auth::commands::login_google_desktop,
            live::commands::youtube_conexao,
            live::commands::youtube_conectar,
            live::commands::youtube_desconectar,
            live::commands::live_estado,
            live::commands::live_espectadores,
            live::commands::live_iniciar,
            live::commands::live_encerrar,
            live::commands::live_abrir,
            chat::chat_abrir_link,
        ])
        .setup(|app| {
            // Treino em dupla: os eventos do motor (`SHMUPX_NETPLAY`) vão para a interface.
            steam::dupla::registrar_app(app.handle());
            // Interface nova publicada para esta versão: entra agora (se já baixada) e é procurada.
            interface::iniciar(app.handle());
            // Log do sistema (arquivo na pasta de logs do app): os detalhes técnicos das falhas ficam
            // aqui, e a interface mostra ao jogador só o que ele pode fazer. Na versão instalada, só
            // avisos e erros.
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(if cfg!(debug_assertions) { log::LevelFilter::Info } else { log::LevelFilter::Warn })
                    .build(),
            )?;

            capture::ffmpeg::usar_empacotado(app.path().resource_dir().ok());
            live::ytdlp::usar_empacotado(app.path().resource_dir().ok());
            input::spawn_listener(app.handle().clone());
            portatil::limpar_atualizacoes();

            Ok(())
        })
        .build(context)
        .expect("error while building tauri application")
        .run(|app, event| {
            // O motor escondido da tela de controles não pode sobrar depois do app fechar.
            if let tauri::RunEvent::Exit = event {
                steam::controles::encerrar(app);
            }
        });
}
