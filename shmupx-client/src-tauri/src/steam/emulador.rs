//! Biblioteca de emulador (port do `FightcadeGameLibraryProvider` do Client antigo): acha a
//! instalação do Fightcade (FBNeo, `emulator/fbneo/fcadefbneo.exe`) e lista os ROMs `.zip` que o
//! jogador tem. "Ter o jogo" no emulador = ter o ROM; quem cruza ROM com o catálogo é a interface.
//! Só lê arquivos deste PC; nada é enviado.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize)]
pub struct BibliotecaEmulador {
    pub found: bool,
    /// Pasta do Fightcade encontrada (para mostrar ao jogador).
    pub root: Option<String>,
    /// Nomes curtos dos ROMs (`ket`, `sonicwi2`…), sem `.zip`, em minúsculas e sem repetição.
    pub roms: Vec<String>,
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
    for letra in b'C'..=b'Z' {
        bases.push(PathBuf::from(format!("{}:\\", letra as char)));
    }
    bases.into_iter().flat_map(|base| [base.join("Fightcade"), base.join("Fightcade2")]).collect()
}

/// Raiz do Fightcade: a pasta informada (a raiz ou o próprio `fcadefbneo.exe`) ou a primeira
/// candidata com o FBNeo dentro.
fn raiz(informada: Option<&str>) -> Option<PathBuf> {
    if let Some(caminho) = informada.map(str::trim).filter(|c| !c.is_empty()) {
        let caminho = PathBuf::from(caminho);
        let raiz = if caminho.file_name().is_some_and(|n| n.eq_ignore_ascii_case("fcadefbneo.exe")) {
            caminho.parent()?.parent()?.parent()?.to_path_buf()
        } else {
            caminho
        };
        return executavel(&raiz).is_file().then_some(raiz);
    }
    candidatas().into_iter().find(|c| executavel(c).is_file())
}

/// Pastas de ROMs do FBNeo dentro da instalação (e a de Documentos, usada pelo Fightcade).
fn pastas_de_roms(raiz: &Path) -> Vec<PathBuf> {
    let mut pastas = vec![
        raiz.join("emulator").join("fbneo").join("ROMs"),
        raiz.join("emulator").join("fbneo").join("roms"),
        raiz.join("ROMs").join("FBNeo ROMs"),
    ];
    if let Some(perfil) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        pastas.push(perfil.join("Documents").join("Fightcade").join("ROMs").join("FBNeo ROMs"));
    }
    pastas
}

pub fn ler(informada: Option<&str>) -> BibliotecaEmulador {
    let Some(raiz) = raiz(informada) else { return BibliotecaEmulador { found: false, root: None, roms: vec![] } };
    let mut roms: Vec<String> = Vec::new();
    for pasta in pastas_de_roms(&raiz) {
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
    BibliotecaEmulador { found: true, root: Some(raiz.to_string_lossy().into_owned()), roms }
}

/// Nome curto de ROM do FBNeo (`sonicwi2`, `md_xxx`): vai como argumento do emulador, então só
/// letras minúsculas, dígitos e `_`.
fn rom_valido(rom: &str) -> bool {
    !rom.is_empty() && rom.len() <= 32 && rom.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
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

/// Abre o jogo com o primeiro ROM da lista que o jogador tem: no ShmupX Engine, se houver
/// (`motor`), senão no FBNeo do Fightcade (`fcadefbneo.exe <rom>`, a partir da pasta do emulador,
/// como no Client antigo). Em tela cheia; com `janela`, em janela. Devolve o PID e o ROM aberto.
pub fn abrir(informada: Option<&str>, roms: &[String], janela: bool, motor: Option<&Motor>) -> Result<(u32, String), String> {
    let roms: Vec<String> = roms.iter().map(|r| r.trim().to_lowercase()).filter(|r| rom_valido(r)).collect();
    if roms.is_empty() {
        return Err("o Client ainda não sabe abrir este jogo no emulador".into());
    }
    let Some(raiz) = raiz(informada) else {
        return Err("Fightcade não encontrado neste computador: instale o Fightcade ou indique a pasta dele em Configurações › Bibliotecas".into());
    };
    let pastas = pastas_de_roms(&raiz);
    if let Some(motor) = motor {
        return abrir_no_motor(motor, &pastas, &roms, janela);
    }
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

    #[test]
    fn pasta_informada_sem_fbneo_nao_vale() {
        assert!(raiz(Some("C:\\pasta-que-nao-existe")).is_none());
    }

    #[test]
    fn so_nomes_de_rom_viram_argumento() {
        assert!(rom_valido("sonicwi2"));
        assert!(rom_valido("md_thunderf4"));
        assert!(!rom_valido(""));
        assert!(!rom_valido("../x"));
        assert!(!rom_valido("ket -w"));
        assert!(abrir(None, &["../../x".into()], false, None).is_err());
    }

    /// Lê o Fightcade deste PC: `cargo test -- --ignored fightcade_deste_pc --nocapture`.
    #[test]
    #[ignore = "depende do Fightcade instalado"]
    fn fightcade_deste_pc() {
        let b = ler(None);
        println!("found={} root={:?} roms={} {:?}", b.found, b.root, b.roms.len(), b.roms.iter().take(12).collect::<Vec<_>>());
    }
}
