//! Como o Client valida as runs ranked de cada jogo do catálogo: pela memória no ShmupX Emulator
//! (`jogos-pela-memoria.json`: o placar vem do jogo, sem script de vídeo) e/ou pelo script de vídeo
//! aprovado (`scripts-aceitos.json`). O Lab só mostra; quem decide são esses arquivos do Client.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::instalador::pasta_client;

/// Validação ranked de um jogo, como o Client a faz.
#[derive(Debug, Default, Serialize, PartialEq)]
pub struct ValidacaoRanked {
    pub game: String,
    /// ROMs com mapa de memória no ShmupX Emulator (vazio: o jogo não vale pela memória).
    pub roms: Vec<String>,
    /// Versão do script de vídeo aceita pelo Client, se houver.
    pub script: Option<String>,
}

#[derive(Deserialize)]
struct PelaMemoria {
    game: String,
    rom: String,
}

#[derive(Deserialize)]
struct ScriptAceito {
    game: String,
    version: String,
}

fn ler_lista<T: for<'de> Deserialize<'de>>(pasta: &Path, arquivo: &str) -> Result<Vec<T>, String> {
    let caminho = pasta.join("src-tauri").join(arquivo);
    let texto = std::fs::read_to_string(&caminho).map_err(|e| format!("falha ao ler {}: {e}", caminho.display()))?;
    serde_json::from_str(&texto).map_err(|e| format!("{arquivo} inválido: {e}"))
}

/// Junta as duas listas do Client (`pasta`: a do `shmupx-client`), um item por jogo, em ordem de id.
pub fn ler(pasta: &Path) -> Result<Vec<ValidacaoRanked>, String> {
    let memoria: Vec<PelaMemoria> = ler_lista(pasta, "jogos-pela-memoria.json")?;
    let scripts: Vec<ScriptAceito> = ler_lista(pasta, "scripts-aceitos.json")?;
    fn do_jogo<'a>(jogos: &'a mut Vec<ValidacaoRanked>, game: &str) -> &'a mut ValidacaoRanked {
        let i = match jogos.iter().position(|j| j.game == game) {
            Some(i) => i,
            None => {
                jogos.push(ValidacaoRanked { game: game.to_string(), ..Default::default() });
                jogos.len() - 1
            }
        };
        &mut jogos[i]
    }
    let mut jogos: Vec<ValidacaoRanked> = Vec::new();
    for item in memoria {
        do_jogo(&mut jogos, &item.game).roms.push(item.rom);
    }
    for item in scripts {
        do_jogo(&mut jogos, &item.game).script = Some(item.version);
    }
    jogos.sort_by(|a, b| a.game.cmp(&b.game));
    Ok(jogos)
}

/// Validação ranked de cada jogo, lida das listas do Client (catálogo do Lab).
#[tauri::command]
pub async fn validacao_ranked_dos_jogos() -> Result<Vec<ValidacaoRanked>, String> {
    let pasta = pasta_client()?;
    tauri::async_runtime::spawn_blocking(move || ler(&pasta)).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junta_memoria_e_script_por_jogo() {
        let pasta = std::env::temp_dir().join(format!("shmupx-lab-ranked-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pasta);
        std::fs::create_dir_all(pasta.join("src-tauri")).unwrap();
        std::fs::write(
            pasta.join("src-tauri/jogos-pela-memoria.json"),
            r#"[{ "game": "aero-fighters-2", "rom": "sonicwi2" }, { "game": "ketsui", "rom": "ket" }]"#,
        )
        .unwrap();
        std::fs::write(
            pasta.join("src-tauri/scripts-aceitos.json"),
            r#"[{ "game": "aero-fighters-2", "version": "0.2.0" }, { "game": "earthion", "version": "1.0.0" }]"#,
        )
        .unwrap();
        let jogos = ler(&pasta).unwrap();
        assert_eq!(
            jogos,
            vec![
                ValidacaoRanked { game: "aero-fighters-2".into(), roms: vec!["sonicwi2".into()], script: Some("0.2.0".into()) },
                ValidacaoRanked { game: "earthion".into(), roms: vec![], script: Some("1.0.0".into()) },
                ValidacaoRanked { game: "ketsui".into(), roms: vec!["ket".into()], script: None },
            ]
        );
        std::fs::remove_file(pasta.join("src-tauri/jogos-pela-memoria.json")).unwrap();
        assert!(ler(&pasta).is_err());
        let _ = std::fs::remove_dir_all(&pasta);
    }

    #[test]
    fn le_as_listas_do_repositorio() {
        let pasta = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shmupx-client");
        let jogos = ler(&pasta).unwrap();
        let darius = jogos.iter().find(|j| j.game == "darius-gaiden").unwrap();
        assert_eq!(darius.roms, ["dariusg"]);
    }
}
