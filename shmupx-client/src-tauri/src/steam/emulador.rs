//! Biblioteca do emulador: a pasta de ROMs (`.zip`) do jogador. Os jogos abrem no ShmupX Emulator
//! (ShmupX Engine, que vem com o Client); "ter o jogo" = ter o ROM, e quem cruza ROM com o catálogo
//! é a interface. A busca automática também olha as pastas de ROMs de instalações antigas do
//! Fightcade, onde muitos jogadores já guardam os ROMs. Só lê arquivos deste PC; nada é enviado.

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
    /// A pasta usada veio da escolha do jogador ("Selecionar pasta de ROMs"), não da busca automática.
    pub manual: bool,
    /// Pasta escolhida pelo jogador e guardada, se houver (mesmo que não esteja disponível agora).
    pub pasta_escolhida: Option<String>,
    /// Havia uma pasta escolhida, mas ela não tem mais ROMs (disco removido?): busca automática.
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

/// Lê os ROMs. `escolhida`: a pasta guardada pelo jogador; `informada`: a que a interface conhece;
/// `emulador`: o ShmupX Emulator está nesta instalação (ver [`motor`]).
pub fn ler(escolhida: Option<&str>, informada: Option<&str>, emulador: bool) -> BibliotecaEmulador {
    let Resolvida { inst, manual, indisponivel } = resolver(escolhida, informada);
    let pasta_escolhida = preenchido(escolhida).map(str::to_string);
    // Para o jogador: a pasta de ROMs (a escolhida, ou a primeira com ROMs da instalação achada).
    let root = inst.roms.clone().or_else(|| pastas_de_roms(&inst).into_iter().find(|p| tem_roms(p))).or_else(|| inst.raiz.clone());
    let Some(root) = root else {
        return BibliotecaEmulador { pasta_escolhida, indisponivel, emulador, ..BibliotecaEmulador::nao_encontrada() };
    };
    let mut roms: Vec<String> = Vec::new();
    for pasta in pastas_de_roms(&inst) {
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

/// Guarda (ou, com `None`, esquece) a pasta escolhida; sem ela, volta a busca automática.
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
}

/// Como o ShmupX Engine mostra o jogo no PC (Configurações › Emulador). Valores ausentes ficam
/// com o padrão: em janela, com scanlines, na resolução nativa do monitor.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VideoDoMotor {
    pub windowed: bool,
    pub scanlines: bool,
    /// `native`: resolução do monitor do PC; `crt15`: TV de tubo de 15 kHz (240p) ligada no PC.
    pub resolution: String,
}

impl Default for VideoDoMotor {
    fn default() -> Self {
        Self { windowed: true, scanlines: true, resolution: "native".into() }
    }
}

/// Cadeia de efeitos (bgfx) das scanlines, publicada junto do motor em `bgfx/chains/`.
const CADEIA_SCANLINES: &str = "shmupx-scanlines";

/// Argumentos de vídeo do motor para as preferências do jogador. `janela` (gravação que precisa
/// da janela) força o modo janela. Scanlines só com a cadeia presente; numa TV de tubo, nunca (a
/// TV já desenha as linhas).
fn argumentos_de_video(video: &VideoDoMotor, janela: bool, scanlines_disponiveis: bool) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    args.push(if janela || video.windowed { "-window" } else { "-nowindow" }.into());
    let crt = video.resolution == "crt15";
    // O GroovyMAME vem configurado para TV de tubo (generic_15); num monitor comum isso trocaria o
    // monitor para 640x480. `lcd` mantém a resolução nativa do monitor.
    if !crt {
        args.extend(["-monitor".into(), "lcd".into()]);
    }
    if video.scanlines && !crt && scanlines_disponiveis {
        args.extend(["-video".into(), "bgfx".into(), "-bgfx_screen_chains".into(), CADEIA_SCANLINES.into()]);
    }
    args
}

const MOTOR_EXE: &str = "shmupx-engine.exe";

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
    Some(Motor { exe, dados, ranking: false, video: VideoDoMotor::default(), controles: vec![] })
}

/// Nome curto de ROM (`sonicwi2`, `md_xxx`): vai como argumento do emulador, então só
/// letras minúsculas, dígitos e `_`.
fn rom_valido(rom: &str) -> bool {
    !rom.is_empty() && rom.len() <= 32 && rom.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// O ShmupX Emulator não veio nesta instalação (para o jogador).
pub const SEM_EMULADOR: &str = "o ShmupX Emulator não foi encontrado nesta instalação: reinstale o ShmupX Client";

/// Abre o jogo no ShmupX Emulator com o primeiro ROM da lista que o jogador tem. Em tela cheia;
/// com `janela`, em janela. Devolve o PID e o ROM aberto.
pub fn abrir(escolhida: Option<&str>, informada: Option<&str>, roms: &[String], janela: bool, motor: Option<&Motor>) -> Result<(u32, String), String> {
    let roms: Vec<String> = roms.iter().map(|r| r.trim().to_lowercase()).filter(|r| rom_valido(r)).collect();
    if roms.is_empty() {
        return Err("o Client ainda não sabe abrir este jogo no emulador".into());
    }
    let motor = motor.ok_or(SEM_EMULADOR)?;
    let Resolvida { inst, .. } = resolver(escolhida, informada);
    abrir_no_motor(motor, &pastas_de_roms(&inst), &roms, janela)
}

/// Abre no ShmupX Engine: `shmupx-engine <rom> -rompath <pastas>` com o plugin de telemetria. Os
/// eventos dele (`SHMUPX {...}`) saem na saída padrão; por enquanto só aparecem no terminal do
/// modo de desenvolvimento.
fn abrir_no_motor(motor: &Motor, pastas: &[PathBuf], roms: &[String], janela: bool) -> Result<(u32, String), String> {
    let pastas: Vec<&PathBuf> = pastas.iter().filter(|p| p.is_dir()).collect();
    let Some(rom) = roms.iter().find(|rom| pastas.iter().any(|p| p.join(format!("{rom}.zip")).is_file())) else {
        return Err(format!("o ROM deste jogo ({}) não está nas pastas de ROMs deste computador", roms.join(" ou ")));
    };
    let rompath = pastas.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>().join(";");
    for sub in ["cfg", "nvram", "snap", "ini"] {
        std::fs::create_dir_all(motor.dados.join(sub)).map_err(|e| format!("não foi possível preparar a pasta do ShmupX Engine: {e}"))?;
    }
    let mut comando = std::process::Command::new(&motor.exe);
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
    let pasta_do_motor = motor.exe.parent().map(Path::to_path_buf).unwrap_or_default();
    let scanlines_disponiveis = pasta_do_motor.join("bgfx").join("chains").join(format!("{CADEIA_SCANLINES}.json")).is_file();
    if motor.video.scanlines && !scanlines_disponiveis {
        log::warn!("emulador: scanlines pedidas, mas o ShmupX Engine não tem bgfx/chains/{CADEIA_SCANLINES}.json; abrindo sem");
    }
    comando.args(argumentos_de_video(&motor.video, janela, scanlines_disponiveis));
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
    comando.stdout(std::process::Stdio::piped());
    let mut filho = comando.spawn().map_err(|e| format!("não foi possível abrir o ShmupX Engine: {e}"))?;
    if let Some(saida) = filho.stdout.take() {
        super::telemetria::acompanhar(filho.id(), saida);
    }
    Ok((filho.id(), rom.clone()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn video_do_motor_vira_argumentos() {
        let padrao = VideoDoMotor::default();
        assert_eq!(
            argumentos_de_video(&padrao, false, true),
            ["-window", "-monitor", "lcd", "-video", "bgfx", "-bgfx_screen_chains", "shmupx-scanlines"]
        );
        // Sem a cadeia no pacote, abre sem scanlines em vez de falhar.
        assert_eq!(argumentos_de_video(&padrao, false, false), ["-window", "-monitor", "lcd"]);
        let tela_cheia = VideoDoMotor { windowed: false, scanlines: false, ..padrao.clone() };
        assert_eq!(argumentos_de_video(&tela_cheia, false, true), ["-nowindow", "-monitor", "lcd"]);
        // A gravação que precisa da janela vence a tela cheia.
        assert_eq!(argumentos_de_video(&tela_cheia, true, true)[0], "-window");
        // TV de tubo: resolução do GroovyMAME (15 kHz) e nada de scanlines por cima.
        let crt = VideoDoMotor { resolution: "crt15".into(), ..padrao };
        assert_eq!(argumentos_de_video(&crt, false, true), ["-window"]);
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
        let lida = ler(Some(&roms.to_string_lossy()), None, true);
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
        let lida = ler(Some("C:\\pasta-que-nao-existe"), Some(&raiz.to_string_lossy()), false);
        assert!(lida.indisponivel && !lida.manual && lida.found && !lida.emulador);
        assert!(lida.roms.contains(&"dariusg".to_string()) && lida.roms.contains(&"ket".to_string()));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn so_nomes_de_rom_viram_argumento() {
        assert!(rom_valido("sonicwi2"));
        assert!(rom_valido("md_thunderf4"));
        assert!(!rom_valido(""));
        assert!(!rom_valido("../x"));
        assert!(!rom_valido("ket -w"));
        assert!(abrir(None, None, &["../../x".into()], false, None).is_err());
        // Sem o ShmupX Emulator, nada abre (não há outro emulador).
        assert_eq!(abrir(None, None, &["sonicwi2".into()], false, None), Err(SEM_EMULADOR.to_string()));
    }

    /// Lê os ROMs deste PC: `cargo test -- --ignored roms_deste_pc --nocapture`.
    #[test]
    #[ignore = "depende dos ROMs deste PC"]
    fn roms_deste_pc() {
        let b = ler(None, None, true);
        println!("found={} root={:?} roms={} {:?}", b.found, b.root, b.roms.len(), b.roms.iter().take(12).collect::<Vec<_>>());
    }
}
