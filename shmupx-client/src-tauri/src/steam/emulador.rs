//! Biblioteca do emulador: a pasta de ROMs (`.zip`) do jogador. Os jogos abrem no ShmupX Emulator
//! (ShmupX Engine, que vem com o Client); "ter o jogo" = ter o ROM, e quem cruza ROM com o catálogo
//! é a interface. Por padrão os ROMs ficam em `roms`, ao lado do Client instalado (o instalador cria
//! a pasta vazia); o jogador pode escolher outra. A busca automática também olha as pastas de ROMs
//! de instalações antigas do Fightcade, onde muitos jogadores já guardam os ROMs. Só lê arquivos
//! deste PC; nada é enviado.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize)]
pub struct BibliotecaEmulador {
    pub found: bool,
    /// Pasta dos ROMs encontrada (para mostrar ao jogador).
    pub root: Option<String>,
    /// Nomes curtos dos ROMs (`ket`, `sonicwi2`…), sem `.zip`, em minúsculas e sem repetição.
    pub roms: Vec<String>,
    /// A pasta usada veio da escolha do jogador ("Selecionar pasta de ROMs"), não é a padrão.
    pub manual: bool,
    /// Pasta escolhida pelo jogador e guardada, se houver (mesmo que não esteja disponível agora).
    pub pasta_escolhida: Option<String>,
    /// Havia uma pasta escolhida, mas ela não tem mais ROMs (disco removido?): volta à padrão.
    pub indisponivel: bool,
    /// O ShmupX Emulator está nesta instalação do Client, então dá para abrir os jogos.
    pub emulador: bool,
}

impl BibliotecaEmulador {
    pub fn nao_encontrada() -> Self {
        BibliotecaEmulador { found: false, root: None, roms: vec![], manual: false, pasta_escolhida: None, indisponivel: false, emulador: false }
    }
}

const EXECUTAVEL: [&str; 3] = ["emulator", "fbneo", "fcadefbneo.exe"];

fn executavel(raiz: &Path) -> PathBuf {
    EXECUTAVEL.iter().fold(raiz.to_path_buf(), |p, parte| p.join(parte))
}

/// Pastas de costume de instalações antigas do Fightcade (com os ROMs do jogador), mais a raiz de cada disco.
fn candidatas() -> Vec<PathBuf> {
    let mut bases: Vec<PathBuf> = ["USERPROFILE", "LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"]
        .iter()
        .filter_map(|v| std::env::var_os(v).map(PathBuf::from))
        .collect();
    if let Some(perfil) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        bases.insert(0, perfil.join("Documents"));
    }
    if cfg!(windows) {
        for letra in b'C'..=b'Z' {
            bases.push(PathBuf::from(format!("{}:\\", letra as char)));
        }
    }
    bases.into_iter().flat_map(|base| [base.join("Fightcade"), base.join("Fightcade2")]).collect()
}

/// Busca automática: a primeira candidata com o FBNeo dentro.
fn raiz_automatica() -> Option<PathBuf> {
    candidatas().into_iter().find(|c| executavel(c).is_file())
}

/// Onde estão o Fightcade e os ROMs, a partir de uma pasta escolhida (ou da busca automática).
#[derive(Debug, Clone, PartialEq)]
struct Instalacao {
    /// Raiz do Fightcade (com `emulator/fbneo/fcadefbneo.exe`), se achou o emulador.
    raiz: Option<PathBuf>,
    /// Pasta de ROMs escolhida diretamente (fora das pastas de ROMs padrão da instalação).
    roms: Option<PathBuf>,
}

impl Instalacao {
    /// O que guardar como escolha do jogador: a pasta de ROMs escolhida (de onde a raiz é achada
    /// de novo) ou a raiz da instalação.
    fn para_guardar(&self) -> Option<PathBuf> {
        self.roms.clone().or_else(|| self.raiz.clone())
    }
}

/// A pasta tem algum ROM (`.zip`) direto dentro?
fn tem_roms(pasta: &Path) -> bool {
    std::fs::read_dir(pasta)
        .map(|entradas| entradas.flatten().any(|e| e.file_name().to_string_lossy().to_lowercase().ends_with(".zip") && e.path().is_file()))
        .unwrap_or(false)
}

/// Entende a pasta que o jogador escolheu: qualquer pasta com ROMs (`.zip`) ou uma instalação antiga
/// do Fightcade (a raiz, `emulator`, `fbneo`, as pastas de ROMs dela ou a pasta que a contém), de
/// onde os ROMs são lidos. Erro (em português, para o jogador) se não houver ROMs.
fn resolver_escolhida(caminho: &Path) -> Result<Instalacao, String> {
    let caminho = if caminho.is_file() { caminho.parent().unwrap_or(caminho) } else { caminho };
    if !caminho.is_dir() {
        return Err(format!("A pasta {} não existe ou não está acessível agora.", caminho.display()));
    }
    // A própria pasta ou uma acima (até a raiz, saindo de `emulator/fbneo/ROMs`) tem o emulador.
    if let Some(raiz) = caminho.ancestors().take(4).find(|a| executavel(a).is_file()).map(Path::to_path_buf) {
        let padrao = pastas_de_roms(&Instalacao { raiz: Some(raiz.clone()), roms: None });
        let fora_do_padrao = caminho != raiz.as_path() && tem_roms(caminho) && !padrao.iter().any(|p| p == caminho);
        return Ok(Instalacao { raiz: Some(raiz), roms: fora_do_padrao.then(|| caminho.to_path_buf()) });
    }
    // Escolheu a pasta que contém o Fightcade (ex.: `D:\Jogos`).
    if let Some(raiz) = ["Fightcade", "Fightcade2"].iter().map(|n| caminho.join(n)).find(|c| executavel(c).is_file()) {
        return Ok(Instalacao { raiz: Some(raiz), roms: None });
    }
    // Qualquer pasta com ROMs (`.zip`).
    if tem_roms(caminho) {
        return Ok(Instalacao { raiz: None, roms: Some(caminho.to_path_buf()) });
    }
    Err(format!("Não achamos ROMs (arquivos .zip) em {}. Escolha a pasta onde estão os ROMs dos seus jogos.", caminho.display()))
}

fn preenchido(caminho: Option<&str>) -> Option<&str> {
    caminho.map(str::trim).filter(|c| !c.is_empty())
}

struct Resolvida {
    inst: Instalacao,
    manual: bool,
    indisponivel: bool,
}

/// Ordem: a pasta escolhida pelo jogador (guardada), a informada pela interface (onde os ROMs
/// foram lidos da última vez) e, por fim, a busca automática.
fn resolver(escolhida: Option<&str>, informada: Option<&str>) -> Resolvida {
    let mut indisponivel = false;
    for (caminho, manual) in [(escolhida, true), (informada, false)] {
        let Some(caminho) = preenchido(caminho) else { continue };
        match resolver_escolhida(Path::new(caminho)) {
            Ok(mut inst) => {
                if inst.raiz.is_none() {
                    inst.raiz = raiz_automatica();
                }
                return Resolvida { inst, manual, indisponivel };
            }
            Err(e) => {
                if manual {
                    log::warn!("emulador: pasta escolhida indisponível: {e}");
                    indisponivel = true;
                }
            }
        }
    }
    Resolvida { inst: Instalacao { raiz: raiz_automatica(), roms: None }, manual: false, indisponivel }
}

/// Pastas de ROMs do FBNeo: a escolhida diretamente, as de dentro da instalação e a de
/// Documentos, usada pelo Fightcade.
fn pastas_de_roms(inst: &Instalacao) -> Vec<PathBuf> {
    let mut pastas: Vec<PathBuf> = inst.roms.iter().cloned().collect();
    if let Some(raiz) = &inst.raiz {
        pastas.push(raiz.join("emulator").join("fbneo").join("ROMs"));
        pastas.push(raiz.join("emulator").join("fbneo").join("roms"));
        pastas.push(raiz.join("ROMs").join("FBNeo ROMs"));
        if let Some(perfil) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
            pastas.push(perfil.join("Documents").join("Fightcade").join("ROMs").join("FBNeo ROMs"));
        }
    }
    pastas
}

/// Pastas onde os ROMs são lidos, em ordem (no `-rompath` do motor, a primeira com o ROM vale): a
/// escolhida pelo jogador, a padrão (`roms` do Client) e as do Fightcade.
fn pastas_para_ler(inst: &Instalacao, padrao: Option<&Path>) -> Vec<PathBuf> {
    let mut pastas: Vec<PathBuf> = inst.roms.iter().cloned().collect();
    if let Some(padrao) = padrao {
        if !pastas.iter().any(|p| p == padrao) {
            pastas.push(padrao.to_path_buf());
        }
    }
    for pasta in pastas_de_roms(&Instalacao { raiz: inst.raiz.clone(), roms: None }) {
        if !pastas.contains(&pasta) {
            pastas.push(pasta);
        }
    }
    pastas
}

/// Pasta padrão dos ROMs: `roms` ao lado do Client instalado (o instalador cria, com escrita liberada
/// aos usuários, para o jogador copiar os ROMs), ou na raiz do ShmupX Portable (fora da pasta da
/// versão, para não sumir na atualização). Se ela não existir nem puder ser criada (Linux, que
/// instala em pasta do sistema), e em desenvolvimento, `roms` na pasta de dados do app.
pub fn pasta_padrao_de_roms(app: &AppHandle) -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        if let Some(instalacao) = crate::portatil::pasta_do_jogador() {
            let pasta = instalacao.join("roms");
            if pasta.is_dir() || std::fs::create_dir_all(&pasta).is_ok() {
                return Some(pasta);
            }
        }
    }
    let pasta = app.path().app_data_dir().ok()?.join("roms");
    std::fs::create_dir_all(&pasta).ok()?;
    Some(pasta)
}

/// Lê os ROMs. `escolhida`: a pasta guardada pelo jogador; `informada`: a que a interface conhece;
/// `padrao`: a pasta padrão (ver [`pasta_padrao_de_roms`]); `emulador`: o ShmupX Emulator está
/// nesta instalação (ver [`motor`]).
pub fn ler(escolhida: Option<&str>, informada: Option<&str>, padrao: Option<&Path>, emulador: bool) -> BibliotecaEmulador {
    let Resolvida { inst, manual, indisponivel } = resolver(escolhida, informada);
    let pasta_escolhida = preenchido(escolhida).map(str::to_string);
    // Para o jogador: a pasta escolhida; sem ela, a padrão (mesmo vazia: é onde ele põe os ROMs) ou,
    // sem padrão, a primeira com ROMs da instalação achada.
    let achada = || inst.roms.clone().or_else(|| pastas_de_roms(&inst).into_iter().find(|p| tem_roms(p))).or_else(|| inst.raiz.clone());
    let root = if manual { achada() } else { padrao.map(Path::to_path_buf).or_else(achada) };
    let Some(root) = root else {
        return BibliotecaEmulador { pasta_escolhida, indisponivel, emulador, ..BibliotecaEmulador::nao_encontrada() };
    };
    let mut roms: Vec<String> = Vec::new();
    for pasta in pastas_para_ler(&inst, padrao) {
        let Ok(entradas) = std::fs::read_dir(&pasta) else { continue };
        for entrada in entradas.flatten() {
            let nome = entrada.file_name().to_string_lossy().to_lowercase();
            if let Some(rom) = nome.strip_suffix(".zip").filter(|r| !r.is_empty()) {
                if !roms.iter().any(|r| r == rom) {
                    roms.push(rom.to_string());
                }
            }
        }
    }
    roms.sort();
    BibliotecaEmulador {
        found: true,
        root: Some(root.to_string_lossy().into_owned()),
        roms,
        manual,
        pasta_escolhida,
        indisponivel,
        emulador,
    }
}

/// Confere a pasta escolhida pelo jogador e devolve o caminho a guardar (a pasta de ROMs, ou a da
/// instalação antiga que tem os ROMs), ou o motivo de ela não servir.
pub fn validar_escolhida(caminho: &str) -> Result<String, String> {
    let caminho = preenchido(Some(caminho)).ok_or_else(|| "Escolha uma pasta.".to_string())?;
    let inst = resolver_escolhida(Path::new(caminho))?;
    Ok(inst.para_guardar().unwrap_or_else(|| PathBuf::from(caminho)).to_string_lossy().into_owned())
}

// Escolha guardada em `<pasta de dados>/fightcade.json` (nome antigo, mantido para não perder a escolha).

#[derive(Serialize, Deserialize, Default)]
struct Config {
    pasta: Option<String>,
}

fn arquivo(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| format!("não foi possível resolver a pasta de dados do app: {e}"))?.join("fightcade.json"))
}

/// Pasta de ROMs escolhida pelo jogador, se houver.
pub fn pasta_escolhida(app: &AppHandle) -> Option<String> {
    let config: Config = arquivo(app).ok().and_then(|a| std::fs::read(a).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    config.pasta.filter(|p| !p.trim().is_empty())
}

/// Guarda (ou, com `None`, esquece) a pasta escolhida; sem ela, volta a pasta padrão.
pub fn guardar_escolhida(app: &AppHandle, pasta: Option<&str>) -> Result<(), String> {
    let arquivo = arquivo(app)?;
    match pasta {
        Some(pasta) => {
            if let Some(dir) = arquivo.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("não foi possível guardar a pasta de ROMs: {e}"))?;
            }
            let json = serde_json::to_vec_pretty(&Config { pasta: Some(pasta.to_string()) }).map_err(|e| e.to_string())?;
            std::fs::write(&arquivo, json).map_err(|e| format!("não foi possível guardar a pasta de ROMs: {e}"))
        }
        None => match std::fs::remove_file(&arquivo) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("não foi possível esquecer a pasta de ROMs: {e}")),
            _ => Ok(()),
        },
    }
}

/// ShmupX Engine (motor de emulação próprio, GroovyMAME): onde está o executável e onde ficam os
/// dados dele (configurações, NVRAM, fotos), fora da pasta do programa (que é só leitura).
#[derive(Debug, Clone)]
pub struct Motor {
    pub exe: PathBuf,
    pub dados: PathBuf,
    /// Partida valendo ranking: abre com `-shmupx_ranked` (velocidade original, sem save state,
    /// menus, cheats ou scripts). No modo treino fica tudo liberado.
    pub ranking: bool,
    /// Preferências de vídeo do jogador (Configurações › Emulador).
    pub video: VideoDoMotor,
    /// Botões do gabinete mapeados pelo jogador (Configurações › Controle).
    pub controles: Vec<super::controles::ControleMapeado>,
    /// Partida Caravan: tempo do Caravan em segundos. O plugin do motor mostra o contador na tela e
    /// fecha o jogo no fim (`SHMUPX_CARAVAN_SEGUNDOS`).
    pub caravan_segundos: Option<u32>,
    /// Caravan: cartão do jogador (avatar e nick) para a tela final (`SHMUPX_JOGADOR_CARTAO`).
    pub cartao_jogador: Option<PathBuf>,
    /// Ranked: arquivo onde o Client grava a posição no ranking, que o motor mostra na tela final
    /// (`SHMUPX_POSICAO_ARQUIVO`).
    pub posicao_arquivo: Option<PathBuf>,
}

/// Como o ShmupX Engine mostra o jogo no PC (Configurações › Emulador). Valores ausentes ficam
/// com o padrão: em janela, com scanlines, na resolução nativa do monitor.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VideoDoMotor {
    pub windowed: bool,
    /// Tela cheia sem borda (borderless), em vez da exclusiva: vale com `windowed` desmarcado.
    pub borderless: bool,
    pub scanlines: bool,
    /// `native`: resolução do monitor do PC; `crt15`: TV de tubo de 15 kHz (240p) ligada no PC.
    pub resolution: String,
    /// Monitor onde o jogo abre, pelo nome do Windows (`\\.\DISPLAY2`); vazio = o principal do
    /// Windows. Monitor que não está mais ligado: o motor abre no principal.
    pub monitor: String,
}

impl Default for VideoDoMotor {
    fn default() -> Self {
        Self { windowed: true, borderless: false, scanlines: true, resolution: "native".into(), monitor: String::new() }
    }
}

/// Nome de monitor do Windows aceito no `-screen` (`\\.\DISPLAY1`…): vai como argumento do motor.
fn monitor_valido(nome: &str) -> bool {
    nome.strip_prefix(r"\\.\DISPLAY").is_some_and(|n| !n.is_empty() && n.len() <= 3 && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Monitor ligado ao PC, para Configurações › Emulador.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MonitorDoPc {
    /// Nome do Windows (`\\.\DISPLAY2`), o valor guardado na preferência.
    pub id: String,
    /// Número do monitor no Windows (`DISPLAY2` → 2), como em Configurações › Tela.
    pub numero: u32,
    pub largura: u32,
    pub altura: u32,
    pub principal: bool,
}

/// Monitores ligados ao PC, do principal para os outros (pela ordem do Windows).
pub fn monitores(app: &AppHandle) -> Vec<MonitorDoPc> {
    let principal = app.primary_monitor().ok().flatten().and_then(|m| m.name().cloned());
    let mut lista: Vec<MonitorDoPc> = app
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
            let id = m.name()?.clone();
            if !monitor_valido(&id) {
                return None;
            }
            let numero = id.trim_start_matches(r"\\.\DISPLAY").parse().ok()?;
            Some(MonitorDoPc { principal: principal.as_deref() == Some(id.as_str()), numero, largura: m.size().width, altura: m.size().height, id })
        })
        .collect();
    lista.sort_by_key(|m| (!m.principal, m.numero));
    lista
}

/// Prefixo das janelas que identificam o monitor (uma por vez).
const JANELA_IDENTIFICAR: &str = "identificar-monitor";
/// Quanto tempo o número fica na tela.
const TEMPO_IDENTIFICAR: std::time::Duration = std::time::Duration::from_secs(3);

/// Mostra o número do monitor (`\\.\DISPLAY2` → 2), grande e em vermelho, no meio daquele monitor
/// por 3 segundos, para o jogador saber qual é qual ao escolher onde os jogos abrem. Janela
/// transparente, sem borda, por cima de tudo, sem foco e sem pegar o mouse; uma nova escolha troca
/// a anterior. `vazio` = o principal do Windows.
pub fn identificar_monitor(app: &AppHandle, id: &str) -> Result<(), String> {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Arc;
    use tauri::{PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

    let monitor = if id.is_empty() {
        app.primary_monitor().ok().flatten()
    } else {
        app.available_monitors().unwrap_or_default().into_iter().find(|m| m.name().map(String::as_str) == Some(id))
    }
    .ok_or("monitor não encontrado")?;
    let numero: u32 = monitor
        .name()
        .filter(|nome| monitor_valido(nome))
        .and_then(|nome| nome.trim_start_matches(r"\\.\DISPLAY").parse().ok())
        .ok_or("monitor sem número")?;

    for (rotulo, janela) in app.webview_windows() {
        if rotulo.starts_with(JANELA_IDENTIFICAR) {
            let _ = janela.destroy();
        }
    }
    static SEQUENCIA: AtomicU64 = AtomicU64::new(0);
    let rotulo = format!("{JANELA_IDENTIFICAR}-{}", SEQUENCIA.fetch_add(1, Ordering::Relaxed));

    // Quadrado no meio do monitor, com metade da menor dimensão dele.
    let (pos, tam) = (monitor.position(), monitor.size());
    let lado = tam.width.min(tam.height) / 2;
    let x = pos.x + (tam.width - lado) as i32 / 2;
    let y = pos.y + (tam.height - lado) as i32 / 2;

    // Aparece quando a página termina de carregar e some 3 s depois.
    let mostrada = Arc::new(AtomicBool::new(false));
    let janela = WebviewWindowBuilder::new(app, &rotulo, WebviewUrl::App(format!("identificar-monitor.html?n={numero}").into()))
        .title("ShmupX")
        .visible(false)
        .focused(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .on_page_load(move |janela, carga| {
            if carga.event() != tauri::webview::PageLoadEvent::Finished || mostrada.swap(true, Ordering::SeqCst) {
                return;
            }
            let _ = janela.show();
            std::thread::spawn(move || {
                std::thread::sleep(TEMPO_IDENTIFICAR);
                let _ = janela.destroy();
            });
        })
        .build()
        .map_err(|e| e.to_string())?;
    let _ = janela.set_ignore_cursor_events(true);
    let _ = janela.set_position(PhysicalPosition::new(x, y));
    let _ = janela.set_size(PhysicalSize::new(lado, lado));
    log::info!("monitor: identificando o {numero} em ({x}, {y}), {lado}px");
    Ok(())
}

/// Cadeia de efeitos (bgfx) das scanlines, publicada junto do motor em `bgfx/chains/`.
const CADEIA_SCANLINES: &str = "shmupx-scanlines";

/// Argumentos de vídeo do motor para as preferências do jogador. `janela` força o modo janela.
/// Scanlines só com a cadeia presente; numa TV de tubo, nunca (a TV já desenha as linhas).
/// `gravada` (run ranked validada pelo vídeo): a tela cheia sem scanlines vai pelo bgfx, e não pela
/// tela cheia exclusiva do Direct3D 9, que a captura da janela vê preta.
/// Tela cheia sem borda (`borderless`, no monitor do PC): pelo bgfx, uma janela sem moldura do
/// tamanho do monitor, sem trocar o modo de vídeo (`-noswitchres`): o Alt+Tab não pisca a tela.
/// Precisa dos shaders do bgfx no pacote (`bgfx_disponivel`). Com `-shmupx_borderless`
/// (`motor_sem_borda`: o motor conhece a opção), a janela também não fica por cima de tudo nem
/// minimiza ao perder o foco (tecla Windows, Alt+Tab). Na TV de tubo, a troca para 15 kHz é o
/// próprio modo: lá vale a tela cheia comum.
fn argumentos_de_video(video: &VideoDoMotor, janela: bool, scanlines_disponiveis: bool, bgfx_disponivel: bool, gravada: bool, motor_sem_borda: bool) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    let tela_cheia = !(janela || video.windowed);
    args.push(if tela_cheia { "-nowindow" } else { "-window" }.into());
    let crt = video.resolution == "crt15";
    // O GroovyMAME vem configurado para TV de tubo (generic_15); num monitor comum isso trocaria o
    // monitor para 640x480. `lcd` mantém a resolução nativa do monitor.
    if !crt {
        args.extend(["-monitor".into(), "lcd".into()]);
    }
    let sem_borda = tela_cheia && video.borderless && !crt && bgfx_disponivel;
    if sem_borda {
        args.push("-noswitchres".into());
        if motor_sem_borda {
            args.push("-shmupx_borderless".into());
        }
    }
    if video.scanlines && !crt && scanlines_disponiveis {
        args.extend(["-video".into(), "bgfx".into(), "-bgfx_screen_chains".into(), CADEIA_SCANLINES.into()]);
    } else if tela_cheia && !crt && bgfx_disponivel && (gravada || sem_borda) {
        args.extend(["-video".into(), "bgfx".into()]);
    }
    // Monitor escolhido pelo jogador (em janela e em tela cheia).
    if monitor_valido(&video.monitor) {
        args.extend(["-screen".into(), video.monitor.clone()]);
    }
    args
}

const MOTOR_EXE: &str = "shmupx-engine.exe";

/// Opção do ShmupX Engine para a tela cheia sem borda (patch `0009` do motor).
const OPCAO_SEM_BORDA: &str = "shmupx_borderless";

/// O executável do motor conhece a opção (o nome dela está no binário)? Um motor antigo recusaria
/// a opção desconhecida e não abriria o jogo. Lido uma vez por executável (caminho, tamanho e data).
fn motor_conhece_opcao(exe: &Path, opcao: &str) -> bool {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    type Chave = (PathBuf, u64, Option<std::time::SystemTime>, String);
    static LIDOS: OnceLock<Mutex<HashMap<Chave, bool>>> = OnceLock::new();
    let Ok(meta) = std::fs::metadata(exe) else { return false };
    let chave = (exe.to_path_buf(), meta.len(), meta.modified().ok(), opcao.to_string());
    let lidos = LIDOS.get_or_init(Default::default);
    if let Some(&conhece) = lidos.lock().unwrap_or_else(|e| e.into_inner()).get(&chave) {
        return conhece;
    }
    let conhece = std::fs::read(exe).is_ok_and(|bytes| memchr::memmem::find(&bytes, opcao.as_bytes()).is_some());
    lidos.lock().unwrap_or_else(|e| e.into_inner()).insert(chave, conhece);
    conhece
}

/// Estado inicial da partida ranked de um ROM no pacote do motor (`states/<rom>/shmupx_ranked.sta`):
/// o jogo na tela de start com 1 crédito, a ficha travada.
fn estado_ranked(pasta_do_motor: &Path, rom: &str) -> PathBuf {
    pasta_do_motor.join("states").join(rom).join("shmupx_ranked.sta")
}

/// Acha o ShmupX Engine: junto do app (`engine/` nos recursos do instalador), na pasta de
/// `SHMUPX_ENGINE_DIR` ou, em desenvolvimento, no pacote de teste do repositório `shmupx-engine`
/// (`shmupx-engine/dist/teste`, ao lado do `shmupx-ecosystem`). Sem ele, os jogos de emulador não abrem.
pub fn motor(app: &AppHandle) -> Option<Motor> {
    let mut pastas: Vec<PathBuf> = Vec::new();
    if let Ok(recursos) = app.path().resource_dir() {
        pastas.push(recursos.join("engine"));
    }
    if let Some(pasta) = std::env::var_os("SHMUPX_ENGINE_DIR") {
        pastas.push(PathBuf::from(pasta));
    }
    if cfg!(debug_assertions) {
        pastas.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("..").join("shmupx-engine").join("dist").join("teste"));
    }
    let exe = pastas.into_iter().map(|p| p.join(MOTOR_EXE)).find(|e| e.is_file())?;
    let dados = app.path().app_data_dir().ok()?.join("engine");
    Some(Motor { exe, dados, ranking: false, video: VideoDoMotor::default(), controles: vec![], caravan_segundos: None, cartao_jogador: None, posicao_arquivo: None })
}

/// Nome curto de ROM (`sonicwi2`, `md_xxx`): vai como argumento do emulador, então só
/// letras minúsculas, dígitos e `_`.
fn rom_valido(rom: &str) -> bool {
    !rom.is_empty() && rom.len() <= 32 && rom.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Comando que roda o ShmupX Engine (executável do Windows): direto no Windows; no Linux, pelo
/// Wine, sem o log do Wine na saída (a telemetria sai por ela) e sem as janelas de instalar o Mono
/// e o Gecko na primeira vez (o motor não usa .NET nem navegador).
/// No Windows o motor é um programa de console: sem `CREATE_NO_WINDOW` ele abriria um terminal
/// preto ao lado do jogo.
pub(crate) fn comando_exe(exe: &Path) -> std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut comando = std::process::Command::new(exe);
        comando.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        comando
    }
    #[cfg(not(windows))]
    {
        let mut comando = std::process::Command::new("wine");
        comando.arg(exe).env("WINEDEBUG", "-all").env("WINEDLLOVERRIDES", "mscoree,mshtml=");
        comando
    }
}

/// Erro ao abrir o motor, para o jogador: no Linux, sem o Wine, diz o que instalar.
pub(crate) fn erro_ao_abrir(e: std::io::Error) -> String {
    if !cfg!(windows) && e.kind() == std::io::ErrorKind::NotFound {
        return "no Linux, o ShmupX Emulator roda pelo Wine, que não está instalado: instale o Wine (ex.: sudo apt install wine) e tente de novo".into();
    }
    format!("não foi possível abrir o ShmupX Emulator: {e}")
}

/// O ShmupX Emulator não veio nesta instalação (para o jogador).
pub const SEM_EMULADOR: &str = "o ShmupX Emulator não foi encontrado nesta instalação: reinstale o ShmupX Client";

/// Abre o jogo no ShmupX Emulator com o primeiro ROM da lista que o jogador tem. Em tela cheia;
/// com `janela`, em janela. Devolve o PID e o ROM aberto.
pub fn abrir(escolhida: Option<&str>, informada: Option<&str>, padrao: Option<&Path>, roms: &[String], janela: bool, motor: Option<&Motor>) -> Result<(u32, String), String> {
    let roms: Vec<String> = roms.iter().map(|r| r.trim().to_lowercase()).filter(|r| rom_valido(r)).collect();
    if roms.is_empty() {
        return Err("o Client ainda não sabe abrir este jogo no emulador".into());
    }
    let motor = motor.ok_or(SEM_EMULADOR)?;
    let Resolvida { inst, .. } = resolver(escolhida, informada);
    abrir_no_motor(motor, &pastas_para_ler(&inst, padrao), &roms, janela)
}

/// Abre no ShmupX Engine: `shmupx-engine <rom> -rompath <pastas>` com o plugin de telemetria. Os
/// eventos dele (`SHMUPX {...}`) saem na saída padrão; por enquanto só aparecem no terminal do
/// modo de desenvolvimento.
fn abrir_no_motor(motor: &Motor, pastas: &[PathBuf], roms: &[String], janela: bool) -> Result<(u32, String), String> {
    let pastas: Vec<&PathBuf> = pastas.iter().filter(|p| p.is_dir()).collect();
    let pasta_do_motor = motor.exe.parent().map(Path::to_path_buf).unwrap_or_default();
    // Ranked: só os ROMs com o estado inicial do ShmupX no pacote do motor (o jogo na tela de start
    // com 1 crédito, ficha travada). Sem ele, o jogo começaria do zero com a ficha liberada.
    let roms: Vec<&String> = if motor.ranking {
        let com_estado: Vec<&String> = roms.iter().filter(|rom| estado_ranked(&pasta_do_motor, rom).is_file()).collect();
        if com_estado.is_empty() {
            return Err("este jogo ainda não tem o Ranked nesta versão do ShmupX Emulator; atualize o ShmupX Client".to_string());
        }
        com_estado
    } else {
        roms.iter().collect()
    };
    let Some(rom) = roms.iter().copied().find(|rom| pastas.iter().any(|p| p.join(format!("{rom}.zip")).is_file())) else {
        return Err(format!("o ROM deste jogo ({}) não está nas pastas de ROMs deste computador", roms.iter().map(|r| r.as_str()).collect::<Vec<_>>().join(" ou ")));
    };
    let rompath = pastas.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join(";");
    for sub in ["cfg", "nvram", "snap", "ini"] {
        std::fs::create_dir_all(motor.dados.join(sub)).map_err(|e| format!("não foi possível preparar a pasta do ShmupX Engine: {e}"))?;
    }
    let mut comando = comando_exe(&motor.exe);
    comando
        .arg(rom)
        .arg("-rompath").arg(&rompath)
        .arg("-cfg_directory").arg(motor.dados.join("cfg"))
        .arg("-nvram_directory").arg(motor.dados.join("nvram"))
        .arg("-snapshot_directory").arg(motor.dados.join("snap"))
        .arg("-inipath").arg(motor.dados.join("ini"))
        .args(["-plugin", "shmupx", "-skip_gameinfo"]);
    if motor.ranking {
        comando.arg("-shmupx_ranked");
    }
    // Caravan: o plugin do ShmupX desenha o contador e o resultado no fim do tempo.
    if let Some(segundos) = motor.caravan_segundos.filter(|_| motor.ranking) {
        comando.env("SHMUPX_CARAVAN_SEGUNDOS", segundos.to_string());
    }
    // Tela final da partida ranked (game over ou fim do tempo): o cartão do jogador e a posição.
    if motor.ranking {
        if let Some(cartao) = &motor.cartao_jogador {
            comando.env("SHMUPX_JOGADOR_CARTAO", cartao);
        }
        if let Some(arquivo) = &motor.posicao_arquivo {
            comando.env("SHMUPX_POSICAO_ARQUIVO", arquivo);
        }
    }
    let scanlines_disponiveis = pasta_do_motor.join("bgfx").join("chains").join(format!("{CADEIA_SCANLINES}.json")).is_file();
    if motor.video.scanlines && !scanlines_disponiveis {
        log::warn!("emulador: scanlines pedidas, mas o ShmupX Engine não tem bgfx/chains/{CADEIA_SCANLINES}.json; abrindo sem");
    }
    // Shaders do bgfx no pacote: a tela cheia sem borda e a da run gravada dependem deles.
    let bgfx_disponivel = pasta_do_motor.join("bgfx").is_dir();
    if motor.video.borderless && !motor.video.windowed && !janela && !bgfx_disponivel {
        log::warn!("emulador: tela cheia sem borda pedida, mas o ShmupX Engine não tem bgfx/; abrindo na tela cheia comum");
    }
    // Run ranked validada pelo vídeo (o ROM não tem mapa de memória): o Client grava a janela.
    let gravada = motor.ranking && !super::telemetria::rom_pela_memoria(rom);
    let motor_sem_borda = motor.video.borderless && motor_conhece_opcao(&motor.exe, OPCAO_SEM_BORDA);
    if motor.video.borderless && !motor_sem_borda {
        log::warn!("emulador: este ShmupX Engine não tem -{OPCAO_SEM_BORDA}; a tela cheia sem borda pode sumir ao perder o foco");
    }
    comando.args(argumentos_de_video(&motor.video, janela, scanlines_disponiveis, bgfx_disponivel, gravada, motor_sem_borda));
    // Controles mapeados no Client: arquivo do MAME que vale para todos os jogos. Se falhar, o jogo
    // abre com os controles padrão.
    match super::controles::preparar_ctrlr(&motor.dados, &motor.controles) {
        Ok(args) => {
            comando.args(args);
        }
        Err(e) => log::warn!("emulador: {e}"),
    }
    // O motor acha bgfx/, artwork/ e plugins/ relativos à pasta dele.
    if motor.exe.parent().is_some() {
        comando.current_dir(&pasta_do_motor);
    }
    log::info!("emulador: ShmupX Engine {} com {rom}{}", motor.exe.display(), if motor.ranking { " (ranking)" } else { "" });
    // A saída padrão traz a telemetria (`SHMUPX {...}`): a run ranked usa o placar da memória.
    // Os avisos do motor (ROM incompleto etc.) vão para o log do Client, não para um terminal.
    comando.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut filho = comando.spawn().map_err(erro_ao_abrir)?;
    if let Some(saida) = filho.stdout.take() {
        super::telemetria::acompanhar(filho.id(), saida);
    }
    if let Some(erros) = filho.stderr.take() {
        std::thread::spawn(move || {
            use std::io::BufRead;
            for linha in std::io::BufReader::new(erros).lines().map_while(Result::ok) {
                let linha = linha.trim();
                if !linha.is_empty() {
                    log::info!("ShmupX Engine: {linha}");
                }
            }
        });
    }
    Ok((filho.id(), rom.clone()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn video_do_motor_vira_argumentos() {
        let padrao = VideoDoMotor::default();
        assert_eq!(
            argumentos_de_video(&padrao, false, true, true, false, true),
            ["-window", "-monitor", "lcd", "-video", "bgfx", "-bgfx_screen_chains", "shmupx-scanlines"]
        );
        // Sem a cadeia no pacote, abre sem scanlines em vez de falhar.
        assert_eq!(argumentos_de_video(&padrao, false, false, true, false, true), ["-window", "-monitor", "lcd"]);
        // Tela cheia comum: a do motor; numa run gravada, pelo bgfx (a captura não vê a exclusiva).
        let tela_cheia = VideoDoMotor { windowed: false, scanlines: false, ..padrao.clone() };
        assert_eq!(argumentos_de_video(&tela_cheia, false, true, true, false, true), ["-nowindow", "-monitor", "lcd"]);
        assert_eq!(argumentos_de_video(&tela_cheia, false, true, true, true, true), ["-nowindow", "-monitor", "lcd", "-video", "bgfx"]);
        assert_eq!(argumentos_de_video(&tela_cheia, false, true, false, true, true), ["-nowindow", "-monitor", "lcd"]);
        // A janela forçada vence a tela cheia.
        assert_eq!(argumentos_de_video(&tela_cheia, true, true, true, false, true)[0], "-window");
        // Tela cheia sem borda: bgfx, sem trocar o modo de vídeo; com scanlines, a cadeia junto.
        let sem_borda = VideoDoMotor { borderless: true, ..tela_cheia.clone() };
        assert_eq!(argumentos_de_video(&sem_borda, false, true, true, false, true), ["-nowindow", "-monitor", "lcd", "-noswitchres", "-shmupx_borderless", "-video", "bgfx"]);
        // Motor antigo, sem a opção: só o bgfx sem trocar o modo de vídeo.
        assert_eq!(argumentos_de_video(&sem_borda, false, true, true, false, false), ["-nowindow", "-monitor", "lcd", "-noswitchres", "-video", "bgfx"]);
        assert_eq!(
            argumentos_de_video(&VideoDoMotor { scanlines: true, ..sem_borda.clone() }, false, true, true, false, true),
            ["-nowindow", "-monitor", "lcd", "-noswitchres", "-shmupx_borderless", "-video", "bgfx", "-bgfx_screen_chains", "shmupx-scanlines"]
        );
        // Sem os shaders do bgfx no pacote, fica a tela cheia comum.
        assert_eq!(argumentos_de_video(&sem_borda, false, true, false, false, true), ["-nowindow", "-monitor", "lcd"]);
        // Sem borda só vale em tela cheia: em janela, nada muda.
        assert_eq!(argumentos_de_video(&VideoDoMotor { windowed: true, ..sem_borda.clone() }, false, true, true, false, true), ["-window", "-monitor", "lcd"]);
        // TV de tubo: resolução do GroovyMAME (15 kHz, com a troca de modo) e nada de scanlines por cima.
        let crt = VideoDoMotor { resolution: "crt15".into(), ..padrao.clone() };
        assert_eq!(argumentos_de_video(&crt, false, true, true, false, true), ["-window"]);
        assert_eq!(argumentos_de_video(&VideoDoMotor { windowed: false, borderless: true, ..crt.clone() }, false, true, true, false, true), ["-nowindow"]);
        // Monitor escolhido vai no -screen; nome estranho é ignorado (abre no principal).
        let segundo = VideoDoMotor { monitor: r"\\.\DISPLAY2".into(), scanlines: false, ..padrao.clone() };
        assert_eq!(argumentos_de_video(&segundo, false, true, true, false, true), ["-window", "-monitor", "lcd", "-screen", r"\\.\DISPLAY2"]);
        let estranho = VideoDoMotor { monitor: r"\\.\DISPLAY2 -x".into(), scanlines: false, ..padrao };
        assert_eq!(argumentos_de_video(&estranho, false, true, true, false, true), ["-window", "-monitor", "lcd"]);
    }

    #[test]
    fn so_nomes_de_monitor_do_windows() {
        assert!(monitor_valido(r"\\.\DISPLAY1") && monitor_valido(r"\\.\DISPLAY12"));
        assert!(!monitor_valido("") && !monitor_valido(r"\\.\DISPLAY") && !monitor_valido("DISPLAY1") && !monitor_valido(r"\\.\DISPLAY1;x"));
    }

    #[test]
    fn video_do_motor_aceita_campos_ausentes() {
        let v: VideoDoMotor = serde_json::from_str(r#"{"scanlines":false}"#).unwrap();
        assert_eq!(v, VideoDoMotor { scanlines: false, ..VideoDoMotor::default() });
    }

    use super::*;

    /// Monta uma instalação falsa do Fightcade numa pasta temporária.
    fn fightcade_falso(nome: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("shmupx-fc-teste-{}-{nome}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let raiz = base.join("Fightcade");
        let fbneo = raiz.join("emulator").join("fbneo");
        std::fs::create_dir_all(fbneo.join("ROMs")).unwrap();
        std::fs::write(fbneo.join("fcadefbneo.exe"), b"").unwrap();
        std::fs::write(fbneo.join("ROMs").join("dariusg.zip"), b"").unwrap();
        std::fs::create_dir_all(raiz.join("ROMs").join("FBNeo ROMs")).unwrap();
        std::fs::write(raiz.join("ROMs").join("FBNeo ROMs").join("ket.zip"), b"").unwrap();
        base
    }

    #[test]
    fn pasta_inexistente_nao_vale() {
        assert!(resolver_escolhida(Path::new("C:\\pasta-que-nao-existe")).is_err());
        assert!(validar_escolhida("   ").is_err());
    }

    #[test]
    fn entende_raiz_emulator_fbneo_roms_e_exe() {
        let base = fightcade_falso("subpastas");
        let raiz = base.join("Fightcade");
        let fbneo = raiz.join("emulator").join("fbneo");
        let esperado = Instalacao { raiz: Some(raiz.clone()), roms: None };
        for escolhida in [
            raiz.clone(),
            raiz.join("emulator"),
            fbneo.clone(),
            fbneo.join("ROMs"),
            raiz.join("ROMs").join("FBNeo ROMs"),
            fbneo.join("fcadefbneo.exe"),
            base.clone(), // a pasta que contém a `Fightcade`
        ] {
            assert_eq!(resolver_escolhida(&escolhida), Ok(esperado.clone()), "{}", escolhida.display());
        }
        assert_eq!(validar_escolhida(&fbneo.to_string_lossy()), Ok(raiz.to_string_lossy().into_owned()));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn pasta_so_de_roms_vale_para_ler() {
        let base = fightcade_falso("so-roms");
        let roms = base.join("MeusROMs");
        std::fs::create_dir_all(&roms).unwrap();
        std::fs::write(roms.join("sonicwi2.zip"), b"").unwrap();
        assert_eq!(resolver_escolhida(&roms), Ok(Instalacao { raiz: None, roms: Some(roms.clone()) }));
        let lida = ler(Some(&roms.to_string_lossy()), None, None, true);
        assert!(lida.found && lida.manual && !lida.indisponivel && lida.emulador);
        assert_eq!(lida.root.as_deref(), Some(&*roms.to_string_lossy()));
        assert!(lida.roms.contains(&"sonicwi2".to_string()));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn pasta_sem_roms_da_erro_claro() {
        let base = std::env::temp_dir().join(format!("shmupx-fc-teste-{}-vazia", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let erro = resolver_escolhida(&base).unwrap_err();
        assert!(erro.contains("Não achamos ROMs"), "{erro}");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn escolhida_que_sumiu_volta_a_busca_automatica() {
        let base = fightcade_falso("informada");
        let raiz = base.join("Fightcade");
        let lida = ler(Some("C:\\pasta-que-nao-existe"), Some(&raiz.to_string_lossy()), None, false);
        assert!(lida.indisponivel && !lida.manual && lida.found && !lida.emulador);
        assert!(lida.roms.contains(&"dariusg".to_string()) && lida.roms.contains(&"ket".to_string()));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn pasta_padrao_vazia_e_a_pasta_do_jogador() {
        let base = fightcade_falso("padrao");
        let padrao = base.join("ShmupX").join("roms");
        std::fs::create_dir_all(&padrao).unwrap();
        // Sem escolha: mostra a padrão (vazia), e os ROMs do Fightcade continuam valendo.
        let lida = ler(None, Some(&base.join("Fightcade").to_string_lossy()), Some(&padrao), true);
        assert!(lida.found && !lida.manual);
        assert_eq!(lida.root.as_deref(), Some(&*padrao.to_string_lossy()));
        assert!(lida.roms.contains(&"ket".to_string()));
        // ROM copiado para a padrão entra na lista.
        std::fs::write(padrao.join("ddp3.zip"), b"").unwrap();
        assert!(ler(None, None, Some(&padrao), true).roms.contains(&"ddp3".to_string()));
        // A escolhida vem antes da padrão.
        let escolhida = base.join("MeusROMs");
        std::fs::create_dir_all(&escolhida).unwrap();
        std::fs::write(escolhida.join("sonicwi2.zip"), b"").unwrap();
        let lida = ler(Some(&escolhida.to_string_lossy()), None, Some(&padrao), true);
        assert!(lida.manual);
        assert_eq!(lida.root.as_deref(), Some(&*escolhida.to_string_lossy()));
        let inst = resolver(Some(&escolhida.to_string_lossy()), None).inst;
        assert_eq!(pastas_para_ler(&inst, Some(&padrao))[..2], [escolhida.clone(), padrao.clone()]);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn so_nomes_de_rom_viram_argumento() {
        assert!(rom_valido("sonicwi2"));
        assert!(rom_valido("md_thunderf4"));
        assert!(!rom_valido(""));
        assert!(!rom_valido("../x"));
        assert!(!rom_valido("ket -w"));
        assert!(abrir(None, None, None, &["../../x".into()], false, None).is_err());
        // Sem o ShmupX Emulator, nada abre (não há outro emulador).
        assert_eq!(abrir(None, None, None, &["sonicwi2".into()], false, None), Err(SEM_EMULADOR.to_string()));
    }

    /// Lê os ROMs deste PC: `cargo test -- --ignored roms_deste_pc --nocapture`.
    #[test]
    #[ignore = "depende dos ROMs deste PC"]
    fn roms_deste_pc() {
        let b = ler(None, None, None, true);
        println!("found={} root={:?} roms={} {:?}", b.found, b.root, b.roms.len(), b.roms.iter().take(12).collect::<Vec<_>>());
    }
}
