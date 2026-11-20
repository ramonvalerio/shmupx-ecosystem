//! Biblioteca local da Steam (port do `SteamLocalLibraryScanner` do Client antigo): acha a
//! instalação, as pastas de biblioteca (`libraryfolders.vdf`) e os jogos instalados
//! (`appmanifest_<appid>.acf`). Só lê arquivos da Steam; nada é enviado a lugar nenhum.

use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct JogoSteam {
    pub app_id: u32,
    pub name: String,
    /// Instalado por completo (`StateFlags` com o bit 4); `false` = baixando/atualizando.
    pub installed: bool,
}

#[derive(Debug, Serialize)]
pub struct Biblioteca {
    /// Steam encontrada no PC.
    pub found: bool,
    pub games: Vec<JogoSteam>,
}

/// Valores `"chave"  "valor"` de um arquivo VDF/ACF (formato texto da Valve), em ordem. Basta
/// para os campos que usamos (`path`, `appid`, `name`, `StateFlags`), que são folhas.
fn pares(texto: &str) -> Vec<(String, String)> {
    let mut saida = Vec::new();
    for linha in texto.lines() {
        let partes: Vec<&str> = linha.split('"').collect();
        // `\t"chave"\t\t"valor"` → ["\t", "chave", "\t\t", "valor", ""]
        if partes.len() >= 5 {
            let valor = partes[3].replace("\\\\", "\\");
            saida.push((partes[1].to_string(), valor));
        }
    }
    saida
}

fn valor<'a>(pares: &'a [(String, String)], chave: &str) -> Option<&'a str> {
    pares.iter().find(|(k, _)| k.eq_ignore_ascii_case(chave)).map(|(_, v)| v.as_str())
}

/// Pasta da Steam: a do registro (`HKCU\Software\Valve\Steam\SteamPath`) ou a padrão.
fn pasta_steam() -> Option<PathBuf> {
    let mut candidatos = Vec::new();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if let Ok(saida) = std::process::Command::new("reg")
            .args(["query", r"HKCU\Software\Valve\Steam", "/v", "SteamPath"])
            .creation_flags(0x0800_0000)
            .output()
        {
            let texto = String::from_utf8_lossy(&saida.stdout);
            if let Some(caminho) = texto.lines().find_map(|l| l.split("REG_SZ").nth(1)).map(str::trim) {
                candidatos.push(PathBuf::from(caminho.replace('/', "\\")));
            }
        }
    }
    // Linux: instalação nativa (`~/.steam/steam` é um link para ela) ou Flatpak.
    #[cfg(not(windows))]
    if let Some(casa) = std::env::var_os("HOME").map(PathBuf::from) {
        candidatos.push(casa.join(".steam/steam"));
        candidatos.push(casa.join(".local/share/Steam"));
        candidatos.push(casa.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
    }
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Ok(base) = std::env::var(var) {
            candidatos.push(Path::new(&base).join("Steam"));
        }
    }
    candidatos.into_iter().find(|c| c.join("steamapps").is_dir())
}

fn pastas_de_biblioteca(steam: &Path) -> Vec<PathBuf> {
    let mut pastas = vec![steam.to_path_buf()];
    for arquivo in [steam.join("steamapps").join("libraryfolders.vdf"), steam.join("config").join("libraryfolders.vdf")] {
        let Ok(texto) = std::fs::read_to_string(&arquivo) else { continue };
        for (chave, caminho) in pares(&texto) {
            if chave.eq_ignore_ascii_case("path") {
                let pasta = PathBuf::from(caminho);
                let repetida = pastas.iter().any(|p| p.to_string_lossy().eq_ignore_ascii_case(&pasta.to_string_lossy()));
                if pasta.is_dir() && !repetida {
                    pastas.push(pasta);
                }
            }
        }
        break;
    }
    pastas
}

fn jogo_do_manifesto(texto: &str) -> Option<JogoSteam> {
    let pares = pares(texto);
    let app_id = valor(&pares, "appid")?.parse().ok()?;
    let name = valor(&pares, "name").unwrap_or_default().to_string();
    let flags: u32 = valor(&pares, "StateFlags").and_then(|f| f.parse().ok()).unwrap_or(0);
    Some(JogoSteam { app_id, name, installed: flags & 4 != 0 })
}

/// Pasta de instalação do jogo (`<biblioteca>/steamapps/common/<installdir>`), se instalado.
pub fn pasta_do_jogo(app_id: u32) -> Option<PathBuf> {
    let steam = pasta_steam()?;
    for pasta in pastas_de_biblioteca(&steam) {
        let manifesto = pasta.join("steamapps").join(format!("appmanifest_{app_id}.acf"));
        let Ok(texto) = std::fs::read_to_string(&manifesto) else { continue };
        let pares = pares(&texto);
        if let Some(dir) = valor(&pares, "installdir").filter(|d| !d.is_empty()) {
            let caminho = pasta.join("steamapps").join("common").join(dir);
            if caminho.is_dir() {
                return Some(caminho);
            }
        }
    }
    None
}

/// Jogos instalados na Steam deste PC (ferramentas como o Steamworks Redistributable incluídas;
/// quem usa filtra pelo catálogo).
pub fn ler() -> Biblioteca {
    let Some(steam) = pasta_steam() else { return Biblioteca { found: false, games: vec![] } };
    let mut games: Vec<JogoSteam> = Vec::new();
    for pasta in pastas_de_biblioteca(&steam) {
        let Ok(entradas) = std::fs::read_dir(pasta.join("steamapps")) else { continue };
        for entrada in entradas.flatten() {
            let nome = entrada.file_name().to_string_lossy().to_string();
            if !(nome.starts_with("appmanifest_") && nome.ends_with(".acf")) {
                continue;
            }
            if let Some(jogo) = std::fs::read_to_string(entrada.path()).ok().as_deref().and_then(jogo_do_manifesto) {
                if !games.iter().any(|g| g.app_id == jogo.app_id) {
                    games.push(jogo);
                }
            }
        }
    }
    games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Biblioteca { found: true, games }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_manifesto_e_pastas() {
        let acf = "\"AppState\"\n{\n\t\"appid\"\t\t\"2181920\"\n\t\"name\"\t\t\"Earthion\"\n\t\"StateFlags\"\t\t\"4\"\n}\n";
        assert_eq!(jogo_do_manifesto(acf), Some(JogoSteam { app_id: 2181920, name: "Earthion".into(), installed: true }));
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}\n";
        assert_eq!(valor(&pares(vdf), "path"), Some("D:\\SteamLibrary"));
        let baixando = acf.replace("\"4\"", "\"1026\"");
        assert!(!jogo_do_manifesto(&baixando).unwrap().installed);
    }
}

#[cfg(test)]
mod neste_pc {
    /// Lê a Steam deste PC de verdade: `cargo test -- --ignored steam_deste_pc --nocapture`.
    #[test]
    #[ignore = "depende da Steam instalada"]
    fn steam_deste_pc() {
        let b = super::ler();
        println!("steam={} jogos={}", b.found, b.games.len());
        for g in b.games.iter().take(8) {
            println!("{} {} {}", g.app_id, g.name, g.installed);
        }
    }
}
