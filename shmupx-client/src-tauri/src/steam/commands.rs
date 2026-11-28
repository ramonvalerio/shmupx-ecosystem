use std::io;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

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
///
/// `sem_steam` (jogo sem a proteção SteamStub, flag No Steam Protection do Lab): abre direto o
/// executável do "Jogar" da Steam, sem abrir a Steam. Se não der (jogo sem a configuração de
/// lançamento na Steam deste PC, ou o executável não abre), cai no `steam://run`.
#[tauri::command]
pub async fn inicializar_jogo_steam(app: AppHandle, steam_app_id: u32, treino: Option<bool>, sem_steam: Option<bool>) -> Result<(), String> {
    if steam_app_id == 0 {
        return Err("steam_app_id inválido (esperado um ID positivo)".into());
    }
    let treino = treino.unwrap_or(false);

    // Processos de antes de abrir: o do jogo é o novo, dentro da pasta dele (aberto direto, ele
    // aparece na hora, antes do monitor começar).
    let baseline = detector::snapshot_pids();

    if sem_steam.unwrap_or(false) {
        let direto = tauri::async_runtime::spawn_blocking(move || super::biblioteca::lancamento_direto(steam_app_id)).await.ok().flatten();
        match direto {
            Some(direto) => match abrir_direto(&direto) {
                Ok(()) => {
                    log::info!("steam: {steam_app_id} aberto direto, sem a Steam: {}", direto.exe.display());
                    spawn_launch_monitor(app, steam_app_id, LAUNCH_MONITOR_TIMEOUT, treino, baseline);
                    return Ok(());
                }
                Err(e) => log::warn!("steam: {steam_app_id} não abriu direto ({}): {e}; abrindo pela Steam", direto.exe.display()),
            },
            None => log::warn!("steam: {steam_app_id} sem o executável do Jogar na Steam deste PC; abrindo pela Steam"),
        }
    }

    // Com a Steam fechada, o `steam://run` a abre e ela inicia o jogo em seguida (como no Client
    // antigo): só é preciso esperar mais pelo processo do jogo.
    let steam_aberta = detector::is_steam_running();

    launch_steam_uri(steam_app_id)
        .map_err(|e| format!("falha ao disparar steam://run/{steam_app_id}: {e}"))?;

    let limite = if steam_aberta { LAUNCH_MONITOR_TIMEOUT } else { LAUNCH_MONITOR_TIMEOUT_STEAM_FECHADA };
    spawn_launch_monitor(app, steam_app_id, limite, treino, baseline);

    Ok(())
}

/// Abre o executável do jogo na pasta de trabalho dele, solto do Client (fechar o Client não fecha o jogo).
fn abrir_direto(direto: &super::biblioteca::LancamentoDireto) -> io::Result<()> {
    let mut comando = std::process::Command::new(&direto.exe);
    comando.args(&direto.argumentos).current_dir(&direto.pasta);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP: sem console e fora do grupo do Client.
        comando.creation_flags(0x0000_0008 | 0x0000_0200);
    }
    comando.spawn().map(|_| ())
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
fn spawn_launch_monitor(app: AppHandle, app_id: u32, limite: Duration, treino: bool, baseline: std::collections::HashSet<u32>) {
    tokio::spawn(async move {
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
                    super::treino::acompanhar(app.clone(), pid, Some(app_id));
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
    // Partida Caravan: o tempo do Caravan do jogo (Lab), para o contador e o fim no motor.
    caravan_segundos: Option<u32>,
    // Partida ranked: a interface salvou o cartão do jogador (salvar_cartao_jogador) para a tela final.
    cartao_jogador: Option<bool>,
    // Idioma do Client (`pt` ou `en`) para os rótulos do cartão de resultado do motor.
    idioma: Option<String>,
    // Treino em dupla online: papel (host/guest), porta e endereços do parceiro (só no treino).
    dupla: Option<super::dupla::Dupla>,
    // Jogo de console (`roms.json`, campo `system`): o sistema do MAME que abre o cartucho.
    sistema: Option<String>,
) -> Result<(), String> {
    let caravan_segundos = caravan_segundos.filter(|s| (30..=3600).contains(s)).filter(|_| !treino.unwrap_or(false));
    let cartao_jogador = if !treino.unwrap_or(false) && cartao_jogador.unwrap_or(false) {
        caminho_do_cartao(&app).filter(|c| c.is_file())
    } else {
        None
    };
    // A imagem da posição é desta partida: a de uma partida anterior não pode aparecer.
    let posicao_arquivo = caminho_da_posicao(&app).filter(|_| !treino.unwrap_or(false));
    if let Some(arquivo) = &posicao_arquivo {
        let _ = std::fs::remove_file(arquivo);
    }
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
            caravan_segundos,
            cartao_jogador,
            posicao_arquivo,
            idioma: idioma.filter(|i| i == "pt" || i == "en"),
            dupla: dupla.filter(|_| treino),
            sistema: sistema.map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty()),
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
    // TV do MiSTer (nogpu): o core Groovy precisa estar aberto e ouvindo antes de o motor mandar a imagem.
    let mister = motor.as_ref().and_then(|m| m.video.mister().map(|host| (host.to_string(), m.video.mister_usuario.clone(), m.video.mister_senha.clone())));
    // Na TV do MiSTer não há janela no PC para gravar: run só de jogo validado pela memória (o
    // placar lido do jogo). O Training vale para qualquer jogo.
    if mister.is_some() && !treino && super::telemetria::roms_pela_memoria(&jogo_id).is_empty() {
        log::warn!("nogpu: {jogo_id} não é validado pela memória; sem run na TV do MiSTer");
        return Err("MISTER_SEM_MEMORIA: este jogo só vale run no monitor do PC".into());
    }
    if let Some((host, usuario, senha)) = &mister {
        let (usuario, senha) = (shmupx_mister_link::usuario_ou_padrao(usuario.as_deref()), shmupx_mister_link::senha_ou_padrao(senha.as_deref()));
        super::mister_ssh::abrir_groovy(host, usuario, senha).await.map_err(|e| {
            log::warn!("nogpu: o MiSTer em {host} não ficou pronto: {e}");
            "MISTER_GROOVY: o MiSTer não ficou pronto para mostrar o jogo na TV".to_string()
        })?;
    }
    let aberto = tauri::async_runtime::spawn_blocking(move || super::emulador::abrir(escolhida.as_deref(), pasta.as_deref(), padrao.as_deref(), &roms, janela, motor.as_ref()))
        .await
        .map_err(|e| e.to_string())?
        .inspect_err(|e| log::warn!("emulador: {e}"))?;
    // Fim da partida (o motor fechou): o MiSTer sai do core Groovy e volta ao menu.
    if let Some((host, usuario, senha)) = mister {
        let pid = aberto.id;
        tauri::async_runtime::spawn(async move {
            while super::detector::processo_vivo_real(pid) {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
            let (usuario, senha) = (shmupx_mister_link::usuario_ou_padrao(usuario.as_deref()), shmupx_mister_link::senha_ou_padrao(senha.as_deref()));
            if let Err(e) = super::mister_ssh::sair_do_groovy(&host, usuario, senha).await {
                log::warn!("nogpu: o MiSTer em {host} não voltou ao menu: {e}");
            }
        });
    }
    // Jogo trocado no ShmupX Emulator aberto (mesma janela): a run do jogo anterior termina agora,
    // antes de a do jogo novo começar.
    if let Some(anterior) = aberto.trocou_de {
        crate::capture::commands::encerrar_run_do_jogo_trocado(&app, anterior).await;
        // Com a live no ar: o jogo ranked novo entra nela; um treino a encerra (não há live fora
        // do Ranked).
        let live = app.state::<crate::live::LiveState>();
        if live.ativa() {
            if treino {
                live.encerrar();
            } else {
                live.jogo_trocado();
            }
        }
    }
    let (pid, rom) = (aberto.id, aberto.rom);
    let _ = app.emit("emulator://game-launched", EmuladorAbertoPayload { game_id: jogo_id, pid, rom, treino });
    if !treino {
        // Partida ranked: quando o motor pausar o jogo e mostrar o resultado, a run se encerra.
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                if !super::detector::processo_vivo(pid) {
                    return;
                }
                if super::telemetria::resultado_mostrado(pid) && crate::capture::commands::encerrar_run_pelo_resultado(&app, pid).await {
                    return;
                }
            }
        });
    }
    if treino {
        super::treino::acompanhar(app.clone(), pid, None);
    }
    Ok(())
}

/// Jogos instalados na Steam deste PC (`libraryfolders.vdf` + `appmanifest_*.acf`).
#[tauri::command]
pub async fn steam_biblioteca() -> super::biblioteca::Biblioteca {
    let b = tauri::async_runtime::spawn_blocking(super::biblioteca::ler)
        .await
        .unwrap_or(super::biblioteca::Biblioteca { found: false, games: vec![] });
    log::info!("biblioteca: Steam {} ({} jogos, {} instalados)", if b.found { "encontrada" } else { "não encontrada" }, b.games.len(), b.games.iter().filter(|g| g.installed).count());
    b
}

/// ROMs deste PC para o ShmupX Emulator. Usa a pasta escolhida pelo jogador (guardada) ou a padrão
/// (`roms` do Client), mais `pasta` (onde a interface leu da última vez) e a busca automática do
/// Fightcade. `emulador` diz se o ShmupX Emulator está nesta instalação.
#[tauri::command]
pub async fn emulador_biblioteca(app: AppHandle, pasta: Option<String>) -> super::emulador::BibliotecaEmulador {
    let escolhida = super::emulador::pasta_escolhida(&app);
    let padrao = super::emulador::pasta_padrao_de_roms(&app);
    let emulador = super::emulador::motor(&app).is_some();
    let b = tauri::async_runtime::spawn_blocking(move || super::emulador::ler(escolhida.as_deref(), pasta.as_deref(), padrao.as_deref(), emulador))
        .await
        .unwrap_or_else(|_| super::emulador::BibliotecaEmulador::nao_encontrada());
    log::info!("biblioteca: ROMs {} ({} ROMs em {})", if b.found { "lidos" } else { "não encontrados" }, b.roms.len(), b.root.as_deref().unwrap_or("-"));
    b
}

/// Monitores ligados ao PC, para escolher onde o ShmupX Emulator abre os jogos (Configurações ›
/// Emulador).
#[tauri::command]
pub fn emulador_monitores(app: AppHandle) -> Vec<super::emulador::MonitorDoPc> {
    super::emulador::monitores(&app)
}

/// Cartão do jogador (avatar e nick, PNG gerado pela interface) para a tela final do Caravan.
pub(crate) fn caminho_do_cartao(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_cache_dir().ok().map(|pasta| pasta.join("caravana-jogador.png"))
}

/// Imagem da posição no ranking (medalha no top 3 e "1º LUGAR") para a tela final do motor.
pub(crate) fn caminho_da_posicao(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_cache_dir().ok().map(|pasta| pasta.join("resultado-posicao.png"))
}

/// Grava um PNG (base64, até 4 MB) no `caminho`, de forma atômica: o motor lê o arquivo enquanto o
/// painel está aberto e nunca pode pegá-lo pela metade.
fn gravar_png(caminho: &std::path::Path, png_base64: &str) -> Result<(), String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(png_base64.as_bytes()).map_err(|e| format!("imagem inválida: {e}"))?;
    if bytes.len() > 4 * 1024 * 1024 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("imagem inválida: não é um PNG".into());
    }
    if let Some(pasta) = caminho.parent() {
        std::fs::create_dir_all(pasta).map_err(|e| e.to_string())?;
    }
    let temporario = caminho.with_extension("tmp");
    std::fs::write(&temporario, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temporario, caminho).map_err(|e| e.to_string())
}

/// Guarda o cartão do jogador (avatar e nick) que o motor mostra na tela final da partida ranked.
#[tauri::command]
pub fn salvar_cartao_jogador(app: AppHandle, png_base64: String) -> Result<(), String> {
    gravar_png(&caminho_do_cartao(&app).ok_or("sem pasta de cache do app")?, &png_base64)
}

/// Guarda a imagem da posição no ranking, que chega depois que o servidor aceita a run; o motor a
/// mostra abaixo do FIM DE JOGO assim que o arquivo aparece.
#[tauri::command]
pub fn salvar_posicao_jogador(app: AppHandle, png_base64: String) -> Result<(), String> {
    gravar_png(&caminho_da_posicao(&app).ok_or("sem pasta de cache do app")?, &png_base64)
}

/// Imagem de outro site (o avatar do Google, por exemplo) como `data:` URL, para a interface poder
/// desenhá-la num canvas e exportar o cartão. Só https e só imagens, até 3 MB.
#[tauri::command]
pub async fn imagem_como_data_url(url: String) -> Result<String, String> {
    use base64::Engine;
    if !url.starts_with("https://") {
        return Err("só imagens https".into());
    }
    let resposta = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("falha ao baixar a imagem: {e}"))?;
    if !resposta.status().is_success() {
        return Err(format!("imagem indisponível ({})", resposta.status()));
    }
    let tipo = resposta.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    if !tipo.starts_with("image/") {
        return Err("o endereço não é uma imagem".into());
    }
    let bytes = resposta.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > 3 * 1024 * 1024 {
        return Err("imagem grande demais".into());
    }
    Ok(format!("data:{tipo};base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes)))
}

/// Mostra o número do monitor no meio dele por 3 segundos, ao escolher o monitor dos jogos.
/// Assíncrono: no Windows, criar janela num comando síncrono (thread principal) trava o `build()`.
#[tauri::command]
pub async fn emulador_identificar_monitor(app: AppHandle, id: String) -> Result<(), String> {
    super::emulador::identificar_monitor(&app, &id).inspect_err(|e| log::warn!("monitor: falha ao identificar {id:?}: {e}"))
}

/// Id do jogo → ROM, pelo `roms.json` da pasta do Client (com a lista padrão): a única ligação entre
/// os jogos do Lab e os ROMs. Ver [`super::roms_auto`].
#[tauri::command]
pub async fn emulador_lista_roms(app: AppHandle) -> std::collections::BTreeMap<String, super::roms_auto::RomDoJogo> {
    let Some(padrao) = super::emulador::pasta_da_lista(&app) else {
        log::warn!("roms: sem pasta de ROMs para ler o roms.json");
        return Default::default();
    };
    tauri::async_runtime::spawn_blocking(move || super::roms_auto::roms_dos_jogos(&padrao)).await.unwrap_or_default()
}

/// Baixa para a pasta do `roms.json` (a escolhida pelo jogador ou a padrão) os ROMs da lista que o
/// jogador ainda não tem em nenhuma pasta lida (a escolhida, a padrão e as do Fightcade). Só os de
/// jogos publicados: `permitidos` (os ids do catálogo, da interface). Ver [`super::roms_auto`].
#[tauri::command]
pub async fn emulador_baixar_roms(app: AppHandle, permitidos: Option<Vec<String>>) -> super::roms_auto::ResultadoDownload {
    let Some(padrao) = super::emulador::pasta_padrao_de_roms(&app) else {
        log::warn!("roms: sem pasta padrão de ROMs para baixar");
        return super::roms_auto::ResultadoDownload::default();
    };
    // Destino: a pasta do `roms.json` (a escolhida pelo jogador ou a padrão).
    let destino = super::emulador::pasta_da_lista(&app).unwrap_or_else(|| padrao.clone());
    // Só na pasta padrão o Client cria o `roms.json`; na escolhida, só baixa se o jogador pôs o arquivo nela.
    let criar_lista = destino == padrao;
    let escolhida = super::emulador::pasta_escolhida(&app);
    let pasta = padrao.clone();
    let tem = tauri::async_runtime::spawn_blocking(move || super::emulador::ler(escolhida.as_deref(), None, Some(&pasta), false).roms)
        .await
        .unwrap_or_default();
    let emitir = |p: super::roms_auto::Progresso| {
        let _ = app.emit("roms://progresso", p);
    };
    super::roms_auto::baixar_faltando(&destino, &tem, &permitidos.unwrap_or_default(), criar_lista, &emitir).await
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

/// MiSTer já conectado: atualiza o agente do ShmupX quando há versão nova publicada (só com o MiSTer
/// no menu; mantém jogadores, tokens e jogos). `Some(versão)` quando atualizou.
#[tauri::command]
pub async fn mister_atualizar_agente(host: String, usuario: Option<String>, senha: Option<String>) -> Result<Option<String>, String> {
    let (usuario, senha) = acesso(&usuario, &senha);
    let resultado = super::mister_ssh::atualizar_agente(host.trim(), &usuario, &senha).await;
    match &resultado {
        Ok(Some(versao)) => log::info!("mister: agente do ShmupX atualizado para {versao} em {host}"),
        Ok(None) => {}
        Err(e) => log::warn!("mister: atualizar o agente em {host}: {e}"),
    }
    resultado
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
    let resultado = tauri::async_runtime::spawn_blocking(move || super::mister::ler(&host, usuario.as_deref(), senha.as_deref()))
        .await
        .map_err(|e| e.to_string())?;
    match &resultado {
        Ok(b) => log::info!("biblioteca: MiSTer {} com {} .mra, {} ROMs e {} jogos de console", b.host, b.mras.len(), b.roms.len(), b.consoles.len()),
        Err(e) => log::warn!("biblioteca: MiSTer não lido: {e}"),
    }
    resultado
}

/// Jogos do Lab marcados com MiSTer que o MiSTer tem: entram no menu ShmupX dele (só acrescenta).
#[tauri::command]
pub async fn mister_sincronizar_menu(host: String, usuario: Option<String>, senha: Option<String>, jogos: Vec<shmupx_mister_link::menu::JogoParaMenu>) -> Result<Vec<String>, String> {
    let (usuario, senha) = acesso(&usuario, &senha);
    shmupx_mister_link::menu::sincronizar(host.trim(), &usuario, &senha, &jogos).await
}
