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
    /// Modo treino: o jogo abriu, mas a interface não inicia a run (nem gasta ficha).
    treino: bool,
}

#[derive(Debug, Clone, Serialize)]
struct GameLaunchTimeoutPayload {
    app_id: u32,
}

/// Inicializa um jogo via protocolo oficial da Steam (`steam://run/<app_id>`), garantindo que
/// overlay e conquistas funcionem normalmente (ver docs/adr/001, bounded context "Sessão de Jogo").
#[tauri::command]
pub async fn inicializar_jogo_steam(app: AppHandle, steam_app_id: u32, treino: Option<bool>) -> Result<(), String> {
    if steam_app_id == 0 {
        return Err("steam_app_id inválido (esperado um ID positivo)".into());
    }

    // Com a Steam fechada, o `steam://run` a abre e ela inicia o jogo em seguida (como no Client
    // antigo): só é preciso esperar mais pelo processo do jogo.
    let steam_aberta = detector::is_steam_running();

    launch_steam_uri(steam_app_id)
        .map_err(|e| format!("falha ao disparar steam://run/{steam_app_id}: {e}"))?;

    let limite = if steam_aberta { LAUNCH_MONITOR_TIMEOUT } else { LAUNCH_MONITOR_TIMEOUT_STEAM_FECHADA };
    spawn_launch_monitor(app, steam_app_id, limite, treino.unwrap_or(false));

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
fn spawn_launch_monitor(app: AppHandle, app_id: u32, limite: Duration, treino: bool) {
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
                    GameLaunchedPayload { app_id, pid, name, treino },
                );
                if treino {
                    super::treino::acompanhar(app.clone(), pid);
                }
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
    /// Modo treino: sem run nem ficha.
    treino: bool,
}

/// Inicializa um jogo de emulador no ShmupX Emulator (ShmupX Engine). `roms`: os ROMs que correspondem ao jogo
/// (a interface cruza com o catálogo); `pasta`: a de ROMs que a interface conhece (a escolhida
/// pelo jogador em "Selecionar pasta de ROMs", guardada no app, vem antes). Avisa
/// `emulator://game-launched` com o processo do emulador, que a gravação vigia como na Steam.
#[tauri::command]
pub async fn inicializar_jogo_emulador(
    app: AppHandle,
    jogo_id: String,
    roms: Vec<String>,
    pasta: Option<String>,
    treino: Option<bool>,
    janela: Option<bool>,
    motor_video: Option<super::emulador::VideoDoMotor>,
    motor_controles: Option<Vec<super::controles::ControleMapeado>>,
) -> Result<(), String> {
    let janela = janela.unwrap_or(false);
    let escolhida = super::emulador::pasta_escolhida(&app);
    let padrao = super::emulador::pasta_padrao_de_roms(&app);
    let treino = treino.unwrap_or(false);
    // ShmupX Engine: partida valendo ranking abre com as travas do modo ranking; treino, sem elas.
    let motor = super::emulador::motor(&app)
        .map(|m| super::emulador::Motor {
            ranking: !treino,
            video: motor_video.unwrap_or_default(),
            controles: motor_controles.unwrap_or_default(),
            ..m
        });
    // Ranked de jogo validado só pela memória (sem script de vídeo): só com o ROM mapeado (outra
    // versão do jogo não tem mapa e a run não teria como ser validada).
    let roms = if treino || crate::analysis::offline::versao_aceita(&jogo_id).is_some() {
        roms
    } else {
        match super::telemetria::roms_pela_memoria(&jogo_id) {
            mapeados if mapeados.is_empty() => roms,
            mapeados => mapeados,
        }
    };
    let (pid, rom) = tauri::async_runtime::spawn_blocking(move || super::emulador::abrir(escolhida.as_deref(), pasta.as_deref(), padrao.as_deref(), &roms, janela, motor.as_ref()))
        .await
        .map_err(|e| e.to_string())?
        .inspect_err(|e| log::warn!("emulador: {e}"))?;
    let _ = app.emit("emulator://game-launched", EmuladorAbertoPayload { game_id: jogo_id, pid, rom, treino });
    if treino {
        super::treino::acompanhar(app.clone(), pid);
    }
    Ok(())
}

/// Jogos instalados na Steam deste PC (`libraryfolders.vdf` + `appmanifest_*.acf`).
#[tauri::command]
pub async fn steam_biblioteca() -> super::biblioteca::Biblioteca {
    tauri::async_runtime::spawn_blocking(super::biblioteca::ler)
        .await
        .unwrap_or(super::biblioteca::Biblioteca { found: false, games: vec![] })
}

/// ROMs deste PC para o ShmupX Emulator. Usa a pasta escolhida pelo jogador (guardada) ou a padrão
/// (`roms` do Client), mais `pasta` (onde a interface leu da última vez) e a busca automática do
/// Fightcade. `emulador` diz se o ShmupX Emulator está nesta instalação.
#[tauri::command]
pub async fn emulador_biblioteca(app: AppHandle, pasta: Option<String>) -> super::emulador::BibliotecaEmulador {
    let escolhida = super::emulador::pasta_escolhida(&app);
    let padrao = super::emulador::pasta_padrao_de_roms(&app);
    let emulador = super::emulador::motor(&app).is_some();
    tauri::async_runtime::spawn_blocking(move || super::emulador::ler(escolhida.as_deref(), pasta.as_deref(), padrao.as_deref(), emulador))
        .await
        .unwrap_or_else(|_| super::emulador::BibliotecaEmulador::nao_encontrada())
}

/// Baixa para a pasta padrão (`roms` do Client) os ROMs de `roms.json` que o jogador ainda não tem
/// em nenhuma pasta lida (a escolhida, a padrão e as do Fightcade). Ver [`super::roms_auto`].
#[tauri::command]
pub async fn emulador_baixar_roms(app: AppHandle) -> super::roms_auto::ResultadoDownload {
    let Some(padrao) = super::emulador::pasta_padrao_de_roms(&app) else {
        log::warn!("roms: sem pasta padrão de ROMs para baixar");
        return super::roms_auto::ResultadoDownload::default();
    };
    let escolhida = super::emulador::pasta_escolhida(&app);
    let pasta = padrao.clone();
    let tem = tauri::async_runtime::spawn_blocking(move || super::emulador::ler(escolhida.as_deref(), None, Some(&pasta), false).roms)
        .await
        .unwrap_or_default();
    super::roms_auto::baixar_faltando(&padrao, &tem).await
}

/// Guarda a pasta de ROMs escolhida pelo jogador e já lê os ROMs dela. Com `pasta` vazia, esquece a
/// escolha e volta à pasta padrão. Se a pasta não tem ROMs, devolve o motivo e não guarda nada.
#[tauri::command]
pub async fn emulador_definir_pasta(app: AppHandle, pasta: Option<String>) -> Result<super::emulador::BibliotecaEmulador, String> {
    let pasta = pasta.filter(|p| !p.trim().is_empty());
    let guardar = match pasta {
        Some(pasta) => Some(
            tauri::async_runtime::spawn_blocking(move || super::emulador::validar_escolhida(&pasta))
                .await
                .map_err(|e| e.to_string())??,
        ),
        None => None,
    };
    super::emulador::guardar_escolhida(&app, guardar.as_deref())?;
    log::info!("emulador: pasta de ROMs {}", guardar.as_deref().unwrap_or("(padrão)"));
    let padrao = super::emulador::pasta_padrao_de_roms(&app);
    let emulador = super::emulador::motor(&app).is_some();
    tauri::async_runtime::spawn_blocking(move || super::emulador::ler(guardar.as_deref(), None, padrao.as_deref(), emulador))
        .await
        .map_err(|e| e.to_string())
}

/// Acesso ao MiSTer: usuário e senha informados, ou os de fábrica (`root` / `1`).
fn acesso(usuario: &Option<String>, senha: &Option<String>) -> (String, String) {
    (
        shmupx_mister_link::usuario_ou_padrao(usuario.as_deref()).to_string(),
        shmupx_mister_link::senha_ou_padrao(senha.as_deref()).to_string(),
    )
}

/// Jogadores já configurados no `shmupx.ini` do MiSTer.
#[tauri::command]
pub async fn mister_usuarios(host: String, usuario: Option<String>, senha: Option<String>) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || super::mister::usuarios(&host, usuario.as_deref(), senha.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

/// Instala (ou atualiza) o ShmupX no MiSTer por SSH e liga o jogador `username` à conta: pede um
/// token novo ao backend e grava no `shmupx.ini` do MiSTer. O token não volta para a interface.
/// O andamento chega pelo evento `mister://etapa`; devolve a saída do instalador.
#[tauri::command]
pub async fn mister_conectar(
    app: AppHandle,
    sessao: tauri::State<'_, crate::session::SessionState>,
    host: String,
    usuario: Option<String>,
    senha: Option<String>,
    username: String,
) -> Result<String, String> {
    if !super::mister::username_valido(&username) {
        return Err("nome de jogador inválido: use letras, números, _ e -".into());
    }
    let login = sessao.snapshot().token.ok_or_else(|| "entre na sua conta para conectar o MiSTer".to_string())?;
    let resposta = crate::net::backend::chamar_json(&login, reqwest::Method::POST, "/api/mister/tokens/direct", None).await?;
    let token = resposta["token"].as_str().filter(|t| t.starts_with("sxm_")).ok_or("o servidor não devolveu o token do MiSTer")?.to_string();
    let (usuario, senha) = acesso(&usuario, &senha);
    let api = crate::net::backend::BACKEND_BASE_URL;
    super::mister_ssh::instalar(host.trim(), &usuario, &senha, &username, &token, api, |etapa| {
        let _ = app.emit("mister://etapa", etapa.to_string());
    })
    .await
}

/// Modos em que um jogador tem cada jogo no menu ShmupX do MiSTer (por SSH).
#[tauri::command]
pub async fn mister_jogos_do_jogador(host: String, usuario: Option<String>, senha: Option<String>, username: String) -> Result<Vec<super::mister_ssh::JogoNoMenu>, String> {
    let (usuario, senha) = acesso(&usuario, &senha);
    super::mister_ssh::jogos_no_menu(host.trim(), &usuario, &senha, &username).await.inspect_err(|e| log::warn!("mister: menu do jogador em {host}: {e}"))
}

/// PARAR JOGO › MiSTer: volta o MiSTer ao menu; o agente registra a partida (com o placar).
#[tauri::command]
pub async fn mister_parar(host: String, usuario: Option<String>, senha: Option<String>) -> Result<(), String> {
    let (usuario, senha) = acesso(&usuario, &senha);
    super::mister_ssh::voltar_ao_menu(host.trim(), &usuario, &senha).await.inspect_err(|e| log::warn!("mister: parar o jogo em {host}: {e}"))
}

/// INICIAR JOGO › MiSTer: abre o jogo do menu ShmupX no MiSTer, já com o jogador e o modo.
#[tauri::command]
pub async fn mister_iniciar(host: String, usuario: Option<String>, senha: Option<String>, caminho: String) -> Result<(), String> {
    let (usuario, senha) = acesso(&usuario, &senha);
    super::mister_ssh::abrir(host.trim(), &usuario, &senha, &caminho).await.inspect_err(|e| log::warn!("mister: abrir {caminho} em {host}: {e}"))
}

/// Falha vista pela interface (abrir o jogo, ler a biblioteca…): o detalhe técnico fica no log do
/// sistema, e o jogador vê só a explicação simples.
#[tauri::command]
pub fn registrar_falha(origem: String, detalhe: String) {
    let detalhe: String = detalhe.chars().take(2000).collect();
    log::warn!("{}: {detalhe}", origem.chars().take(60).collect::<String>());
}

/// Conferência rápida (1,5 s) se o MiSTer responde na rede, antes de abrir ou parar um jogo nele.
/// `None` = respondeu; senão, o motivo para a interface (o detalhe técnico vai para o log).
#[tauri::command]
pub async fn mister_alcance(host: String) -> Option<shmupx_mister_link::descoberta::SemConexao> {
    let motivo = shmupx_mister_link::descoberta::alcance(&host).await.err()?;
    log::warn!("mister: {} fora de alcance ({motivo:?}): {}", host.trim(), shmupx_mister_link::descoberta::motivo_sem_conexao(&host));
    Some(motivo)
}

/// Procura o MiSTer na rede local (SSH com o usuário e a senha, padrão `root` / `1`).
#[tauri::command]
pub async fn mister_procurar(usuario: Option<String>, senha: Option<String>) -> Result<Vec<String>, String> {
    let (usuario, senha) = acesso(&usuario, &senha);
    shmupx_mister_link::descoberta::procurar(&usuario, &senha).await
}

/// Jogos do MiSTer FPGA na rede (FTP). `usuario`/`senha`: os do MiSTer, se não forem os padrão.
#[tauri::command]
pub async fn mister_biblioteca(host: String, usuario: Option<String>, senha: Option<String>) -> Result<super::mister::BibliotecaMister, String> {
    tauri::async_runtime::spawn_blocking(move || super::mister::ler(&host, usuario.as_deref(), senha.as_deref()))
        .await
        .map_err(|e| e.to_string())?
}

/// Jogos do Lab marcados com MiSTer que o MiSTer tem: entram no menu ShmupX dele (só acrescenta).
#[tauri::command]
pub async fn mister_sincronizar_menu(host: String, usuario: Option<String>, senha: Option<String>, jogos: Vec<shmupx_mister_link::menu::JogoParaMenu>) -> Result<Vec<String>, String> {
    let (usuario, senha) = acesso(&usuario, &senha);
    shmupx_mister_link::menu::sincronizar(host.trim(), &usuario, &senha, &jogos).await
}
