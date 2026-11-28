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

/// Entende a pasta que o jogador escolheu: qualquer pasta com ROMs (`.zip`) ou com o `roms.json`
/// (os ROMs dele são baixados nela), ou uma instalação antiga do Fightcade (a raiz, `emulator`,
/// `fbneo`, as pastas de ROMs dela ou a pasta que a contém), de onde os ROMs são lidos. Erro (em
/// português, para o jogador) se não houver nenhum dos dois.
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
    // Qualquer pasta com ROMs (`.zip`) ou com a lista de download (`roms.json`).
    if tem_roms(caminho) || super::roms_auto::arquivo_da_lista(caminho).is_file() {
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

/// Pastas onde os ROMs são lidos, em ordem (no `-rompath` do motor, a primeira com o ROM vale).
/// Com pasta escolhida pelo jogador (`manual`), só ela (ou, se ele escolheu uma instalação do
/// Fightcade, só as pastas de ROMs dela). Sem escolha: a padrão (`roms` do Client) e as do
/// Fightcade achado pela busca automática.
fn pastas_para_ler(inst: &Instalacao, padrao: Option<&Path>, manual: bool) -> Vec<PathBuf> {
    if manual {
        return match &inst.roms {
            Some(roms) => vec![roms.clone()],
            None => pastas_de_roms(inst),
        };
    }
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

/// Pasta do `roms.json` e dos ROMs baixados: a escolhida pelo jogador, se é uma pasta de ROMs (não
/// uma instalação do Fightcade) e está acessível; senão, a padrão.
pub fn pasta_da_lista(app: &AppHandle) -> Option<PathBuf> {
    let escolhida = pasta_escolhida(app)
        .and_then(|p| resolver_escolhida(Path::new(&p)).ok())
        .and_then(|inst| inst.roms.filter(|_| inst.raiz.is_none()));
    escolhida.or_else(|| pasta_padrao_de_roms(app))
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
    for pasta in pastas_para_ler(&inst, padrao, manual) {
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
    /// Idioma dos rótulos do cartão de resultado: `pt` ou `en` (`SHMUPX_IDIOMA`).
    pub idioma: Option<String>,
    /// Treino em dupla online (`-shmupx_netplay`): papel, porta e endereços do parceiro.
    pub dupla: Option<super::dupla::Dupla>,
    /// Jogo de console (`roms.json`, campo `system`): o sistema do MAME (`snes`, `genesis`…) que
    /// abre o cartucho (`<sistema> -cart <rom>.zip`). `None`: jogo de arcade, aberto pelo nome do ROM.
    pub sistema: Option<String>,
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
    /// `native`: resolução do monitor do PC; `crt15`: monitor CRT de arcade de 15 kHz (240p) ligado no PC;
    /// `crt31`: monitor CRT de PC de 31 kHz (VGA) ligado no PC;
    /// `mister`: a imagem vai para a TV ligada no MiSTer (nogpu, core Groovy) e os controles vêm de lá.
    pub resolution: String,
    /// Monitor onde o jogo abre, pelo nome do Windows (`\\.\DISPLAY2`); vazio = o principal do
    /// Windows. Monitor que não está mais ligado: o motor abre no principal.
    pub monitor: String,
    /// `mister`: endereço do MiSTer na rede e o acesso SSH (nulos = `root` / `1`), para abrir o core
    /// Groovy antes da partida e voltar ao menu no fim.
    pub mister_host: String,
    pub mister_usuario: Option<String>,
    pub mister_senha: Option<String>,
    /// Play on MiSTer: na TV pelo MiSTer, o controle é o ligado no MiSTer; desmarcado, o do PC
    /// (com o mapeamento do Client).
    pub play_on_mister: bool,
}

impl Default for VideoDoMotor {
    fn default() -> Self {
        Self {
            windowed: true,
            borderless: false,
            scanlines: true,
            resolution: "native".into(),
            monitor: String::new(),
            mister_host: String::new(),
            mister_usuario: None,
            mister_senha: None,
            play_on_mister: false,
        }
    }
}

impl VideoDoMotor {
    /// Imagem na TV do MiSTer (nogpu): o endereço do MiSTer, se é esse o modo e o endereço é válido.
    pub fn mister(&self) -> Option<&str> {
        let host = self.mister_host.trim();
        let valido = !host.is_empty() && host.len() <= 253 && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
        (self.resolution == "mister" && valido).then_some(host)
    }

    /// Controle ligado no MiSTer: na TV pelo MiSTer com Play on MiSTer marcado.
    pub fn controle_do_mister(&self) -> bool {
        self.mister().is_some() && self.play_on_mister
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
/// Tela cheia, só sem borda: nunca a tela cheia exclusiva, que prende a tela. Pelo bgfx, uma janela
/// sem moldura do tamanho do monitor; com `-shmupx_borderless` (`motor_sem_borda`: o motor conhece a
/// opção), ela também não fica por cima de tudo nem minimiza ao perder o foco (tecla Windows,
/// Alt+Tab). Vale também em janela, para quando o jogador passa para a tela cheia no jogo (duplo
/// clique, Alt+Enter). No monitor do PC, sem trocar o modo de vídeo (`-noswitchres`: o Alt+Tab não
/// pisca a tela); na TV de tubo, a troca para 15 kHz continua (sem ela a TV não mostra a imagem).
/// Precisa dos shaders do bgfx no pacote (`bgfx_disponivel`); sem eles, fica o vídeo comum do motor.
/// O bgfx também é o que a captura da run gravada vê (a tela cheia exclusiva sai preta).
fn argumentos_de_video(video: &VideoDoMotor, janela: bool, scanlines_disponiveis: bool, bgfx_disponivel: bool, motor_sem_borda: bool) -> Vec<String> {
    // TV do MiSTer (nogpu): o motor manda cada quadro ao core Groovy, no modo de 15 kHz do jogo.
    // Nada aparece no monitor do PC. `-aspect 4:3`: a TV é 4:3; sem isso o Switchres usa a proporção
    // do monitor do PC (16:9) e estica a imagem (Darius Gaiden em 426x232 em vez dos 320x232
    // originais). Com Play on MiSTer, os controles vêm do MiSTer; sem, ficam os do PC.
    if let Some(host) = video.mister() {
        let mut args: Vec<String> = ["-video", "mister", "-mister_ip", host, "-aspect", "4:3"].map(String::from).to_vec();
        if video.play_on_mister {
            args.extend(["-joystickprovider".into(), "mister".into()]);
        } else {
            // Sem janela no PC, o motor nunca fica em foco: o controle do PC (DirectInput) só chega
            // ao jogo com a leitura em segundo plano.
            args.push("-background_input".into());
        }
        return args;
    }
    let mut args: Vec<String> = Vec::new();
    let tela_cheia = !(janela || video.windowed);
    args.push(if tela_cheia { "-nowindow" } else { "-window" }.into());
    let crt31 = video.resolution == "crt31";
    let crt = video.resolution == "crt15" || crt31;
    // O GroovyMAME vem configurado para TV de tubo (generic_15); num monitor comum isso trocaria o
    // monitor para 640x480. `lcd` mantém a resolução nativa do monitor. Monitor de tubo de PC
    // (31 kHz): `pc_31_120`, o modo do jogo em 31 kHz (240p em 120 Hz, cada quadro mostrado duas vezes).
    if crt31 {
        args.extend(["-monitor".into(), "pc_31_120".into()]);
    } else if !crt {
        args.extend(["-monitor".into(), "lcd".into()]);
    }
    if bgfx_disponivel {
        if !crt {
            args.push("-noswitchres".into());
        }
        if motor_sem_borda {
            args.push("-shmupx_borderless".into());
        }
        args.extend(["-video".into(), "bgfx".into()]);
        if video.scanlines && !crt && scanlines_disponiveis {
            args.extend(["-bgfx_screen_chains".into(), CADEIA_SCANLINES.into()]);
        }
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
    Some(Motor { exe, dados, ranking: false, video: VideoDoMotor::default(), controles: vec![], caravan_segundos: None, cartao_jogador: None, posicao_arquivo: None, idioma: None, dupla: None, sistema: None })
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

/// Jogo aberto no ShmupX Emulator.
#[derive(Debug, Clone, PartialEq)]
pub struct JogoAberto {
    /// Id da partida: o PID do motor ou, num jogo trocado no mesmo processo, um id dele
    /// (`sessao_motor`); vale em tudo que recebe o PID do jogo (vigia, gravação, telemetria).
    pub id: u32,
    pub rom: String,
    /// A partida anterior, quando o jogo foi trocado no motor que já estava aberto (mesma janela).
    pub trocou_de: Option<u32>,
}

/// Abre o jogo no ShmupX Emulator com o primeiro ROM da lista que o jogador tem. Em tela cheia;
/// com `janela`, em janela. Com o motor já aberto, troca o jogo nele (mesma janela); se o motor
/// não aceitar a troca, fecha o anterior e abre outro.
pub fn abrir(escolhida: Option<&str>, informada: Option<&str>, padrao: Option<&Path>, roms: &[String], janela: bool, motor: Option<&Motor>) -> Result<JogoAberto, String> {
    let roms: Vec<String> = roms.iter().map(|r| r.trim().to_lowercase()).filter(|r| rom_valido(r)).collect();
    if roms.is_empty() {
        return Err("o Client ainda não sabe abrir este jogo no emulador".into());
    }
    let motor = motor.ok_or(SEM_EMULADOR)?;
    let Resolvida { inst, manual, .. } = resolver(escolhida, informada);
    let lancamento = preparar_lancamento(motor, &pastas_para_ler(&inst, padrao, manual), &roms, janela)?;
    if !lancamento.dupla {
        match super::sessao_motor::trocar(&lancamento) {
            Ok(Some((id, anterior))) => {
                log::info!("emulador: {} na mesma janela do ShmupX Emulator (partida {anterior} -> {id})", lancamento.rom);
                return Ok(JogoAberto { id, rom: lancamento.rom, trocou_de: Some(anterior) });
            }
            Ok(None) => {}
            Err(e) => log::warn!("emulador: troca de jogo no motor aberto: {e}"),
        }
    }
    // Um motor só: o que estava aberto (sem aceitar a troca, ou na dupla online) fecha antes.
    super::sessao_motor::fechar_aberto();
    let (id, rom) = abrir_processo(motor, lancamento)?;
    Ok(JogoAberto { id, rom, trocou_de: None })
}

/// Opções do motor que mudam de um jogo para outro (`preparar_lancamento` e
/// `argumentos_de_video`). Na troca de jogo no mesmo processo, voltam ao padrão antes de receber
/// as do jogo novo: a que o jogo novo não usa não fica com o valor do anterior.
const OPCOES_DA_PARTIDA: &[&str] = &[
    "rompath", "cfg_directory", "nvram_directory", "snapshot_directory", "skip_gameinfo", "shmupx_ranked",
    "window", "monitor", "switchres", OPCAO_SEM_BORDA, "video", "bgfx_screen_chains", "screen", "ctrlrpath", "ctrlr",
];

/// Variáveis do plugin do ShmupX que mudam de um jogo para outro (Caravan, tela final do ranked).
const AMBIENTE_DA_PARTIDA: &[&str] = &["SHMUPX_CARAVAN_SEGUNDOS", "SHMUPX_JOGADOR_CARTAO", "SHMUPX_POSICAO_ARQUIVO", "SHMUPX_IDIOMA"];

/// Tudo para abrir um jogo no motor: o ROM, os argumentos (depois do ROM) e as variáveis do plugin.
#[derive(Debug, Clone)]
pub struct Lancamento {
    pub rom: String,
    /// Console: o sistema do MAME que abre o cartucho (o `-cart` vai em `args`).
    pub sistema: Option<String>,
    pub args: Vec<String>,
    pub ambiente: Vec<(String, String)>,
    pub pasta_do_motor: PathBuf,
    pub ranking: bool,
    /// Treino em dupla online: sempre num processo novo (a conexão é do processo).
    pub dupla: bool,
}

impl Lancamento {
    /// Linha do comando `trocar` para o plugin do ShmupX (campos separados por TAB). `None`: algum
    /// valor tem TAB ou quebra de linha e não cabe no comando.
    pub fn comando_trocar(&self) -> Option<String> {
        // Console abre num processo novo: o plugin troca só jogos de arcade, pelo nome do ROM.
        if self.sistema.is_some() {
            return None;
        }
        let mut campos = vec!["trocar".to_string(), format!("rom={}", self.rom)];
        campos.extend(OPCOES_DA_PARTIDA.iter().map(|o| format!("padrao={o}")));
        campos.extend(self.args.iter().map(|a| format!("arg={a}")));
        for nome in AMBIENTE_DA_PARTIDA {
            let valor = self.ambiente.iter().find(|(n, _)| n == nome).map_or("", |(_, v)| v.as_str());
            campos.push(format!("env={nome}={valor}"));
        }
        if campos.iter().any(|c| c.contains(['\t', '\r', '\n'])) {
            return None;
        }
        Some(campos.join("\t") + "\n")
    }
}

/// Monta a abertura no ShmupX Engine: `shmupx-engine <rom> -rompath <pastas>` com o plugin de
/// telemetria, as travas do ranked, o vídeo e os controles do jogador.
fn preparar_lancamento(motor: &Motor, pastas: &[PathBuf], roms: &[String], janela: bool) -> Result<Lancamento, String> {
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
    // Console: o sistema do MAME abre o cartucho (o `.zip` do ROM, da primeira pasta que o tem).
    let sistema = match motor.sistema.as_deref() {
        Some(sistema) if !rom_valido(sistema) => return Err(format!("sistema de console inválido: {sistema:?}")),
        outro => outro.map(str::to_string),
    };
    let cartucho = sistema.as_ref().and_then(|_| pastas.iter().map(|p| p.join(format!("{rom}.zip"))).find(|c| c.is_file()));
    for sub in ["cfg", "nvram", "snap", "ini"] {
        std::fs::create_dir_all(motor.dados.join(sub)).map_err(|e| format!("não foi possível preparar a pasta do ShmupX Engine: {e}"))?;
    }
    let caminho = |sub: &str| motor.dados.join(sub).to_string_lossy().into_owned();
    let mut args: Vec<String> = cartucho.iter().flat_map(|c| ["-cart".to_string(), c.to_string_lossy().into_owned()]).collect();
    args.extend([
        "-rompath".into(), rompath,
        "-cfg_directory".into(), caminho("cfg"),
        "-nvram_directory".into(), caminho("nvram"),
        "-snapshot_directory".into(), caminho("snap"),
        "-inipath".into(), caminho("ini"),
        "-plugin".into(), "shmupx".into(), "-skip_gameinfo".into(),
    ]);
    let mut ambiente: Vec<(String, String)> = Vec::new();
    if motor.ranking {
        args.push("-shmupx_ranked".into());
    }
    // Treino em dupla online: o motor conecta ao parceiro antes do primeiro quadro.
    let dupla = motor.dupla.as_ref().filter(|_| !motor.ranking);
    if let Some(dupla) = dupla {
        dupla.validar()?;
        if !motor_conhece_opcao(&motor.exe, "shmupx_netplay") {
            return Err("este ShmupX Emulator ainda não tem o treino em dupla; atualize o ShmupX Client".to_string());
        }
        args.extend(dupla.argumentos());
    }
    // Caravan: o plugin do ShmupX desenha o contador e o resultado no fim do tempo.
    if let Some(segundos) = motor.caravan_segundos.filter(|_| motor.ranking) {
        ambiente.push(("SHMUPX_CARAVAN_SEGUNDOS".into(), segundos.to_string()));
    }
    // Tela final da partida ranked (game over ou fim do tempo): o cartão do jogador e a posição.
    if motor.ranking {
        if let Some(cartao) = &motor.cartao_jogador {
            ambiente.push(("SHMUPX_JOGADOR_CARTAO".into(), cartao.to_string_lossy().into_owned()));
        }
        if let Some(arquivo) = &motor.posicao_arquivo {
            ambiente.push(("SHMUPX_POSICAO_ARQUIVO".into(), arquivo.to_string_lossy().into_owned()));
        }
        if let Some(idioma) = &motor.idioma {
            ambiente.push(("SHMUPX_IDIOMA".into(), idioma.clone()));
        }
    }
    let scanlines_disponiveis = pasta_do_motor.join("bgfx").join("chains").join(format!("{CADEIA_SCANLINES}.json")).is_file();
    if motor.video.scanlines && !scanlines_disponiveis {
        log::warn!("emulador: scanlines pedidas, mas o ShmupX Engine não tem bgfx/chains/{CADEIA_SCANLINES}.json; abrindo sem");
    }
    // Shaders do bgfx no pacote: a tela cheia sem borda depende deles.
    let bgfx_disponivel = pasta_do_motor.join("bgfx").is_dir();
    if !bgfx_disponivel {
        log::warn!("emulador: o ShmupX Engine não tem bgfx/; a tela cheia não fica sem borda");
    }
    let motor_sem_borda = motor_conhece_opcao(&motor.exe, OPCAO_SEM_BORDA);
    if !motor_sem_borda {
        log::warn!("emulador: este ShmupX Engine não tem -{OPCAO_SEM_BORDA}; a tela cheia pode sumir ao perder o foco");
    }
    args.extend(argumentos_de_video(&motor.video, janela, scanlines_disponiveis, bgfx_disponivel, motor_sem_borda));
    // Controles mapeados no Client: arquivo do MAME que vale para todos os jogos. Se falhar, o jogo
    // abre com os controles padrão.
    // Na TV do MiSTer os controles vêm do MiSTer (o mapeamento dele): o do PC não vale.
    let controles: &[super::controles::ControleMapeado] = if motor.video.controle_do_mister() { &[] } else { &motor.controles };
    match super::controles::preparar_ctrlr(&motor.dados, controles) {
        Ok(extra) => args.extend(extra),
        Err(e) => log::warn!("emulador: {e}"),
    }
    Ok(Lancamento { rom: rom.clone(), sistema, args, ambiente, pasta_do_motor, ranking: motor.ranking, dupla: dupla.is_some() })
}

/// Abre o motor num processo novo. Os eventos do plugin (`SHMUPX {...}`) saem na saída padrão; a
/// entrada padrão fica com o Client, para trocar de jogo no mesmo processo (`sessao_motor`).
fn abrir_processo(motor: &Motor, lancamento: Lancamento) -> Result<(u32, String), String> {
    let mut comando = comando_exe(&motor.exe);
    // Arcade: o nome do ROM; console: o sistema (o cartucho vai no `-cart`).
    comando.arg(lancamento.sistema.as_deref().unwrap_or(&lancamento.rom)).args(&lancamento.args).envs(lancamento.ambiente.iter().map(|(n, v)| (n, v)));
    // O motor acha bgfx/, artwork/ e plugins/ relativos à pasta dele.
    if motor.exe.parent().is_some() {
        comando.current_dir(&lancamento.pasta_do_motor);
    }
    log::info!("emulador: ShmupX Engine {} com {}{}", motor.exe.display(), lancamento.rom, if lancamento.ranking { " (ranking)" } else { "" });
    // A saída padrão traz a telemetria (`SHMUPX {...}`): a run ranked usa o placar da memória.
    // Os avisos do motor (ROM incompleto etc.) vão para o log do Client, não para um terminal.
    comando.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut filho = comando.spawn().map_err(erro_ao_abrir)?;
    let pid = filho.id();
    super::sessao_motor::registrar(pid, filho.stdin.take(), lancamento.dupla);
    if let Some(saida) = filho.stdout.take() {
        super::telemetria::acompanhar(pid, saida);
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
    Ok((pid, lancamento.rom))
}

#[cfg(test)]
mod tests {
    #[test]
    fn video_do_motor_vira_argumentos() {
        let padrao = VideoDoMotor::default();
        let sem_borda = ["-noswitchres", "-shmupx_borderless", "-video", "bgfx"];
        let com = |inicio: &[&str], fim: &[&str]| -> Vec<String> { inicio.iter().chain(fim).map(|s| s.to_string()).collect() };
        // Em janela: já pronta para a tela cheia sem borda (duplo clique, Alt+Enter no jogo).
        assert_eq!(
            argumentos_de_video(&padrao, false, true, true, true),
            com(&["-window", "-monitor", "lcd"], &[&sem_borda[..], &["-bgfx_screen_chains", "shmupx-scanlines"]].concat())
        );
        // Sem a cadeia no pacote, abre sem scanlines em vez de falhar.
        assert_eq!(argumentos_de_video(&padrao, false, false, true, true), com(&["-window", "-monitor", "lcd"], &sem_borda));
        // Tela cheia: sempre sem borda, nunca a exclusiva (também numa preferência antiga sem borderless).
        let tela_cheia = VideoDoMotor { windowed: false, scanlines: false, ..padrao.clone() };
        assert_eq!(argumentos_de_video(&tela_cheia, false, true, true, true), com(&["-nowindow", "-monitor", "lcd"], &sem_borda));
        assert_eq!(argumentos_de_video(&VideoDoMotor { borderless: true, ..tela_cheia.clone() }, false, true, true, true), com(&["-nowindow", "-monitor", "lcd"], &sem_borda));
        // A janela forçada vence a tela cheia.
        assert_eq!(argumentos_de_video(&tela_cheia, true, true, true, true)[0], "-window");
        // Motor antigo, sem a opção: só o bgfx sem trocar o modo de vídeo.
        assert_eq!(argumentos_de_video(&tela_cheia, false, true, true, false), ["-nowindow", "-monitor", "lcd", "-noswitchres", "-video", "bgfx"]);
        // Sem os shaders do bgfx no pacote, fica o vídeo comum do motor.
        assert_eq!(argumentos_de_video(&tela_cheia, false, true, false, true), ["-nowindow", "-monitor", "lcd"]);
        // TV de tubo: sem borda também, mas com a troca para 15 kHz e sem scanlines por cima.
        // TV do MiSTer (nogpu): só a saída de rede e os controles do MiSTer; sem endereço, o vídeo comum.
        let mister = VideoDoMotor { resolution: "mister".into(), mister_host: "192.168.0.7".into(), ..padrao.clone() };
        assert_eq!(argumentos_de_video(&mister, true, true, true, true), ["-video", "mister", "-mister_ip", "192.168.0.7", "-aspect", "4:3", "-background_input"]);
        let com_controle = VideoDoMotor { play_on_mister: true, ..mister.clone() };
        assert_eq!(argumentos_de_video(&com_controle, true, true, true, true), ["-video", "mister", "-mister_ip", "192.168.0.7", "-aspect", "4:3", "-joystickprovider", "mister"]);
        assert_eq!(argumentos_de_video(&VideoDoMotor { mister_host: "x; rm".into(), ..mister.clone() }, false, true, true, true)[0], "-window");
        assert_eq!(argumentos_de_video(&VideoDoMotor { mister_host: String::new(), ..mister }, false, true, true, true)[0], "-window");
        let crt = VideoDoMotor { resolution: "crt15".into(), ..padrao.clone() };
        assert_eq!(argumentos_de_video(&crt, false, true, true, true), ["-window", "-shmupx_borderless", "-video", "bgfx"]);
        assert_eq!(argumentos_de_video(&VideoDoMotor { windowed: false, ..crt.clone() }, false, true, true, true), ["-nowindow", "-shmupx_borderless", "-video", "bgfx"]);
        // Monitor de tubo de PC (31 kHz): troca para o modo do jogo em 31 kHz, sem scanlines por cima.
        let crt31 = VideoDoMotor { resolution: "crt31".into(), windowed: false, ..padrao.clone() };
        assert_eq!(argumentos_de_video(&crt31, false, true, true, true), ["-nowindow", "-monitor", "pc_31_120", "-shmupx_borderless", "-video", "bgfx"]);
        // Monitor escolhido vai no -screen; nome estranho é ignorado (abre no principal).
        let segundo = VideoDoMotor { monitor: r"\\.\DISPLAY2".into(), scanlines: false, ..padrao.clone() };
        assert_eq!(argumentos_de_video(&segundo, false, true, true, true), com(&["-window", "-monitor", "lcd"], &[&sem_borda[..], &["-screen", r"\\.\DISPLAY2"]].concat()));
        let estranho = VideoDoMotor { monitor: r"\\.\DISPLAY2 -x".into(), scanlines: false, ..padrao };
        assert_eq!(argumentos_de_video(&estranho, false, true, true, true), com(&["-window", "-monitor", "lcd"], &sem_borda));
    }

    #[test]
    fn video_do_motor_aceita_campos_ausentes() {
        let v: VideoDoMotor = serde_json::from_str(r#"{"scanlines":false}"#).unwrap();
        assert_eq!(v, VideoDoMotor { scanlines: false, ..VideoDoMotor::default() });
    }

    use super::*;

    #[test]
    fn comando_trocar_leva_o_jogo_as_opcoes_e_o_ambiente() {
        let lancamento = Lancamento {
            rom: "ket".into(),
            sistema: None,
            args: vec!["-rompath".into(), r"D:\roms".into(), "-shmupx_ranked".into()],
            ambiente: vec![("SHMUPX_IDIOMA".into(), "en".into())],
            pasta_do_motor: PathBuf::new(),
            ranking: true,
            dupla: false,
        };
        let linha = lancamento.comando_trocar().unwrap();
        assert!(linha.starts_with("trocar\trom=ket\t"));
        assert!(linha.ends_with('\n'));
        assert!(linha.contains("\tpadrao=shmupx_ranked\t"), "a opção do jogo anterior volta ao padrão");
        assert!(linha.contains("\targ=-rompath\targ=D:\\roms\targ=-shmupx_ranked\t"));
        assert!(linha.contains("\tenv=SHMUPX_IDIOMA=en"));
        // A variável que o jogo novo não usa vai vazia: não fica a do jogo anterior.
        assert!(linha.contains("\tenv=SHMUPX_CARAVAN_SEGUNDOS=\t"));
        let com_tab = Lancamento { args: vec!["-rompath".into(), "D:\\a\tb".into()], ..lancamento };
        assert!(com_tab.comando_trocar().is_none(), "TAB no valor não cabe no comando");
        let console = Lancamento { rom: "axelayu".into(), sistema: Some("snes".into()), args: vec![], ..com_tab };
        assert!(console.comando_trocar().is_none(), "console abre num processo novo");
    }

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
    fn pasta_so_com_roms_json_vale() {
        let base = std::env::temp_dir().join(format!("shmupx-fc-teste-{}-lista", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("roms.json"), "{}").unwrap();
        assert_eq!(resolver_escolhida(&base), Ok(Instalacao { raiz: None, roms: Some(base.clone()) }));
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
        // Com pasta escolhida, só ela é lida: os ROMs da padrão e do Fightcade ficam de fora.
        assert_eq!(lida.roms, ["sonicwi2"]);
        let inst = resolver(Some(&escolhida.to_string_lossy()), None).inst;
        assert_eq!(pastas_para_ler(&inst, Some(&padrao), true), [escolhida.clone()]);
        assert_eq!(pastas_para_ler(&inst, Some(&padrao), false)[..2], [escolhida.clone(), padrao.clone()]);
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
