//! Biblioteca de emulador (port do `FightcadeGameLibraryProvider` do Client antigo): acha a
//! instalação do Fightcade (FBNeo, `emulator/fbneo/fcadefbneo.exe`) e lista os ROMs `.zip` que o
//! jogador tem. "Ter o jogo" no emulador = ter o ROM; quem cruza ROM com o catálogo é a interface.
//! Só lê arquivos deste PC; nada é enviado.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize)]
pub struct BibliotecaEmulador {
    pub found: bool,
    /// Pasta do Fightcade encontrada (para mostrar ao jogador): a raiz da instalação ou, sem o
    /// emulador, a pasta de ROMs escolhida.
    pub root: Option<String>,
    /// Nomes curtos dos ROMs (`ket`, `sonicwi2`…), sem `.zip`, em minúsculas e sem repetição.
    pub roms: Vec<String>,
    /// A pasta usada veio da escolha do jogador ("Selecionar pasta do Fightcade"), não da busca automática.
    pub manual: bool,
    /// Pasta escolhida pelo jogador e guardada, se houver (mesmo que não esteja disponível agora).
    pub pasta_escolhida: Option<String>,
    /// Havia uma pasta escolhida, mas ela não tem mais o Fightcade (disco removido?): busca automática.
    pub indisponivel: bool,
    /// Achou o emulador (`fcadefbneo.exe`), então dá para abrir jogos; sem ele, só a pasta de ROMs.
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

/// Pastas onde o Fightcade costuma ser instalado (as do Client antigo, mais a raiz de cada disco).
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

/// Entende a pasta que o jogador escolheu: a raiz do Fightcade, a `emulator`, a `fbneo`, uma pasta
/// de ROMs (`emulator/fbneo/ROMs`, `ROMs/FBNeo ROMs` ou qualquer outra com `.zip`), o próprio
/// `fcadefbneo.exe` ou a pasta que contém a `Fightcade`. Erro (em português, para o jogador) se
/// ela não parece do Fightcade.
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
    // Só uma pasta de ROMs (sem o emulador por perto): dá para ler os jogos; para abrir, a busca
    // automática ainda tenta achar o emulador.
    if tem_roms(caminho) {
        return Ok(Instalacao { raiz: None, roms: Some(caminho.to_path_buf()) });
    }
    Err(format!(
        "Essa pasta não parece ser do Fightcade: não achamos o emulador (emulator\\fbneo\\fcadefbneo.exe) nem ROMs (.zip) em {}. Escolha a pasta onde o Fightcade foi instalado (ex.: C:\\Fightcade) ou a pasta de ROMs dele.",
        caminho.display()
    ))
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

/// Lê os ROMs. `escolhida`: a pasta guardada pelo jogador; `informada`: a que a interface conhece.
pub fn ler(escolhida: Option<&str>, informada: Option<&str>) -> BibliotecaEmulador {
    let Resolvida { inst, manual, indisponivel } = resolver(escolhida, informada);
    let pasta_escolhida = preenchido(escolhida).map(str::to_string);
    let Some(root) = inst.raiz.clone().or_else(|| inst.roms.clone()) else {
        return BibliotecaEmulador { pasta_escolhida, indisponivel, ..BibliotecaEmulador::nao_encontrada() };
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
        emulador: inst.raiz.is_some(),
    }
}

/// Confere a pasta escolhida pelo jogador e devolve o caminho a guardar (a raiz do Fightcade ou
/// a pasta de ROMs), ou o motivo de ela não servir.
pub fn validar_escolhida(caminho: &str) -> Result<String, String> {
    let caminho = preenchido(Some(caminho)).ok_or_else(|| "Escolha uma pasta.".to_string())?;
    let inst = resolver_escolhida(Path::new(caminho))?;
    Ok(inst.para_guardar().unwrap_or_else(|| PathBuf::from(caminho)).to_string_lossy().into_owned())
}

// Escolha guardada em `<pasta de dados>/fightcade.json` (como a pasta das gravações).

#[derive(Serialize, Deserialize, Default)]
struct Config {
    pasta: Option<String>,
}

fn arquivo(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| format!("não foi possível resolver a pasta de dados do app: {e}"))?.join("fightcade.json"))
}

/// Pasta do Fightcade escolhida pelo jogador, se houver.
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
                std::fs::create_dir_all(dir).map_err(|e| format!("não foi possível guardar a pasta do Fightcade: {e}"))?;
            }
            let json = serde_json::to_vec_pretty(&Config { pasta: Some(pasta.to_string()) }).map_err(|e| e.to_string())?;
            std::fs::write(&arquivo, json).map_err(|e| format!("não foi possível guardar a pasta do Fightcade: {e}"))
        }
        None => match std::fs::remove_file(&arquivo) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("não foi possível esquecer a pasta do Fightcade: {e}")),
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
}

const MOTOR_EXE: &str = "shmupx-engine.exe";

/// Acha o ShmupX Engine: junto do app (`engine/` nos recursos do instalador), na pasta de
/// `SHMUPX_ENGINE_DIR` ou, em desenvolvimento, no pacote de teste do repositório `shmupx-engine`
/// (`shmupx-engine/dist/teste`, ao lado do `shmupx-ecosystem`). Sem ele, o jogo abre no Fightcade.
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
    Some(Motor { exe, dados })
}

/// Nome curto de ROM do FBNeo (`sonicwi2`, `md_xxx`): vai como argumento do emulador, então só
/// letras minúsculas, dígitos e `_`.
fn rom_valido(rom: &str) -> bool {
    !rom.is_empty() && rom.len() <= 32 && rom.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Abre o jogo com o primeiro ROM da lista que o jogador tem: no ShmupX Engine, se houver
/// (`motor`), senão no FBNeo do Fightcade (`fcadefbneo.exe <rom>`, a partir da pasta do emulador,
/// como no Client antigo). Em tela cheia; com `janela`, em janela. Devolve o PID e o ROM aberto.
pub fn abrir(escolhida: Option<&str>, informada: Option<&str>, roms: &[String], janela: bool, motor: Option<&Motor>) -> Result<(u32, String), String> {
    let roms: Vec<String> = roms.iter().map(|r| r.trim().to_lowercase()).filter(|r| rom_valido(r)).collect();
    if roms.is_empty() {
        return Err("o Client ainda não sabe abrir este jogo no emulador".into());
    }
    let Resolvida { inst, .. } = resolver(escolhida, informada);
    if let Some(motor) = motor {
        return abrir_no_motor(motor, &pastas_de_roms(&inst), &roms, janela);
    }
    let Some(raiz) = inst.raiz.clone() else {
        if inst.roms.is_some() {
            return Err("achamos a pasta de ROMs escolhida, mas não o emulador do Fightcade (fcadefbneo.exe): selecione a pasta onde o Fightcade foi instalado em Importar jogos › Emulador".into());
        }
        return Err("Fightcade não encontrado neste computador: instale o Fightcade ou selecione a pasta dele em Importar jogos › Emulador".into());
    };
    let pastas = pastas_de_roms(&inst);
    let Some(rom) = roms.iter().find(|rom| pastas.iter().any(|p| p.join(format!("{rom}.zip")).is_file())) else {
        return Err(format!("o ROM deste jogo ({}) não está no Fightcade: baixe-o pelo Fightcade e tente de novo", roms.join(" ou ")));
    };
    let exe = executavel(&raiz);
    let mut comando = std::process::Command::new(&exe);
    comando.arg(rom);
    if janela {
        comando.arg("-w");
    }
    if let Some(pasta) = exe.parent() {
        comando.current_dir(pasta);
    }
    let filho = comando.spawn().map_err(|e| format!("não foi possível abrir o emulador: {e}"))?;
    Ok((filho.id(), rom.clone()))
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
    if janela {
        comando.arg("-window");
    }
    if let Some(pasta) = motor.exe.parent() {
        comando.current_dir(pasta);
    }
    log::info!("emulador: ShmupX Engine {} com {rom}", motor.exe.display());
    let filho = comando.spawn().map_err(|e| format!("não foi possível abrir o ShmupX Engine: {e}"))?;
    Ok((filho.id(), rom.clone()))
}

#[cfg(test)]
mod tests {
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
        let lida = ler(Some(&roms.to_string_lossy()), None);
        assert!(lida.found && lida.manual && !lida.indisponivel);
        assert!(lida.roms.contains(&"sonicwi2".to_string()));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn pasta_sem_fightcade_da_erro_claro() {
        let base = std::env::temp_dir().join(format!("shmupx-fc-teste-{}-vazia", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let erro = resolver_escolhida(&base).unwrap_err();
        assert!(erro.contains("não parece ser do Fightcade"), "{erro}");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn escolhida_que_sumiu_volta_a_busca_automatica() {
        let base = fightcade_falso("informada");
        let raiz = base.join("Fightcade");
        let lida = ler(Some("C:\\pasta-que-nao-existe"), Some(&raiz.to_string_lossy()));
        assert!(lida.indisponivel && !lida.manual && lida.found && lida.emulador);
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
    }

    /// Lê o Fightcade deste PC: `cargo test -- --ignored fightcade_deste_pc --nocapture`.
    #[test]
    #[ignore = "depende do Fightcade instalado"]
    fn fightcade_deste_pc() {
        let b = ler(None, None);
        println!("found={} root={:?} roms={} {:?}", b.found, b.root, b.roms.len(), b.roms.iter().take(12).collect::<Vec<_>>());
    }
}
