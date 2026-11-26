//! Versão portátil (ShmupX Portable, `shmupx-client/portable`): o lançador descompacta o Client em
//! `%LOCALAPPDATA%\ShmupX Portable\app\<versão>\`, com a marca `.portable` ao lado do exe.
//!
//! O que muda no Client portátil:
//! - `roms` e `recordings` ficam em `ShmupX Portable\`, fora da pasta da versão: a próxima versão
//!   é outra pasta, e os ROMs e vídeos do jogador não podem sumir com ela;
//! - a atualização automática baixa o próximo `ShmupX-Portable-<versão>.exe` (SHA-256 conferido
//!   com o backend) e o abre, em vez de rodar o instalador NSIS em Arquivos de Programas.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

use crate::net::backend::{BACKEND_BASE_URL, VERSAO_CLIENT};

const MARCA: &str = ".portable";
const EVENTO_PROGRESSO: &str = "portatil://progresso";

fn pasta_do_exe() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(Path::to_path_buf)
}

/// `ShmupX Portable\` (duas pastas acima do exe: `app\<versão>\`), se este Client é o portátil.
pub fn raiz() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        return None;
    }
    let exe = pasta_do_exe()?;
    if !exe.join(MARCA).is_file() {
        return None;
    }
    Some(exe.parent()?.parent()?.to_path_buf())
}

/// Onde ficam `roms` e `recordings` por padrão: ao lado do Client instalado, ou na raiz do portátil.
pub fn pasta_do_jogador() -> Option<PathBuf> {
    raiz().or_else(pasta_do_exe)
}

fn pasta_de_atualizacoes() -> Option<PathBuf> {
    Some(raiz()?.join("updates"))
}

fn arquivo_da_versao(versao: &str) -> Option<PathBuf> {
    Some(pasta_de_atualizacoes()?.join(format!("ShmupX-Portable-{versao}.exe")))
}

fn versao_valida(versao: &str) -> bool {
    let partes: Vec<&str> = versao.split('.').collect();
    partes.len() == 3 && partes.iter().all(|p| !p.is_empty() && p.len() <= 4 && p.chars().all(|c| c.is_ascii_digit()))
}

/// Apaga atualizações já aplicadas (o lançador delas já descompactou a versão que está rodando).
pub fn limpar_atualizacoes() {
    let Some(pasta) = pasta_de_atualizacoes() else { return };
    for entrada in std::fs::read_dir(pasta).into_iter().flatten().flatten() {
        let nome = entrada.file_name().to_string_lossy().into_owned();
        let antiga = nome
            .strip_prefix("ShmupX-Portable-")
            .and_then(|r| r.strip_suffix(".exe").or_else(|| r.strip_suffix(".exe.part")))
            .and_then(|v| Some((ler(v)?, ler(VERSAO_CLIENT)?)))
            .is_some_and(|(arquivo, atual)| arquivo <= atual);
        if antiga {
            let _ = std::fs::remove_file(entrada.path());
        }
    }
}

fn ler(versao: &str) -> Option<(u32, u32, u32)> {
    let mut p = versao.split('.').map(|n| n.parse().ok());
    Some((p.next()??, p.next()??, p.next()??))
}

/// Este Client é a versão portátil?
#[tauri::command]
pub fn portatil_ativo() -> bool {
    raiz().is_some()
}

#[derive(Deserialize)]
struct Downloads {
    portable: Option<Portatil>,
}

#[derive(Deserialize)]
struct Portatil {
    version: String,
    url: String,
    sha256: String,
}

#[derive(Clone, Serialize)]
struct Progresso {
    recebido: u64,
    total: Option<u64>,
}

/// Baixa o ShmupX Portable da `versao` (a que o updater ofereceu) e confere o SHA-256 publicado
/// pelo backend. O andamento sai em `portatil://progresso`.
#[tauri::command]
pub async fn portatil_baixar(app: AppHandle, versao: String) -> Result<(), String> {
    if !versao_valida(&versao) {
        return Err("versão inválida".into());
    }
    let destino = arquivo_da_versao(&versao).ok_or("este Client não é a versão portátil")?;
    if destino.is_file() {
        return Ok(());
    }
    let http = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("falha ao preparar o download: {e}"))?;
    let downloads: Downloads = http
        .get(format!("{BACKEND_BASE_URL}/api/client/downloads"))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| format!("backend indisponível: {e}"))?
        .json()
        .await
        .map_err(|e| format!("resposta inesperada do backend: {e}"))?;
    let pacote = downloads.portable.filter(|p| p.version == versao).ok_or_else(|| format!("a versão portátil {versao} ainda não está publicada"))?;
    if !pacote.url.starts_with("https://www.shmupx.com/downloads/") {
        return Err(format!("endereço inesperado para a versão portátil: {}", pacote.url));
    }

    let pasta = destino.parent().ok_or("pasta de atualizações inválida")?;
    std::fs::create_dir_all(pasta).map_err(|e| format!("falha ao criar {}: {e}", pasta.display()))?;
    let parcial = destino.with_extension("exe.part");
    let mut resposta = http.get(&pacote.url).send().await.and_then(reqwest::Response::error_for_status).map_err(|e| format!("falha ao baixar: {e}"))?;
    let total = resposta.content_length();
    let mut arquivo = std::fs::File::create(&parcial).map_err(|e| format!("falha ao gravar {}: {e}", parcial.display()))?;
    let mut hash = Sha256::new();
    let mut recebido = 0u64;
    let mut ultimo_aviso = 0u64;
    while let Some(pedaco) = resposta.chunk().await.map_err(|e| format!("download interrompido: {e}"))? {
        use std::io::Write;
        arquivo.write_all(&pedaco).map_err(|e| format!("falha ao gravar {}: {e}", parcial.display()))?;
        hash.update(&pedaco);
        recebido += pedaco.len() as u64;
        if recebido - ultimo_aviso >= 1 << 20 {
            ultimo_aviso = recebido;
            let _ = app.emit(EVENTO_PROGRESSO, Progresso { recebido, total });
        }
    }
    drop(arquivo);
    let calculado: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if !calculado.eq_ignore_ascii_case(pacote.sha256.trim()) {
        let _ = std::fs::remove_file(&parcial);
        return Err(format!("o arquivo baixado não confere (SHA-256 {calculado}, esperado {})", pacote.sha256));
    }
    std::fs::rename(&parcial, &destino).map_err(|e| format!("falha ao gravar {}: {e}", destino.display()))?;
    Ok(())
}

/// Abre o ShmupX Portable baixado (ele descompacta a versão nova e a abre) e fecha este Client.
#[tauri::command]
pub fn portatil_instalar(app: AppHandle, versao: String) -> Result<(), String> {
    if !versao_valida(&versao) {
        return Err("versão inválida".into());
    }
    let arquivo = arquivo_da_versao(&versao).filter(|a| a.is_file()).ok_or("baixe a versão nova antes de atualizar")?;
    std::process::Command::new(&arquivo)
        .current_dir(arquivo.parent().unwrap_or(Path::new(".")))
        .spawn()
        .map_err(|e| format!("falha ao abrir {}: {e}", arquivo.display()))?;
    app.exit(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versoes() {
        assert!(versao_valida("0.1.18"));
        assert!(!versao_valida("../0.1.18"));
        assert!(!versao_valida("0.1"));
        assert_eq!(ler("0.1.18"), Some((0, 1, 18)));
        assert!(ler("0.1.9") < ler("0.1.18"));
    }

    #[test]
    fn em_desenvolvimento_nunca_e_portatil() {
        // Os testes rodam em debug: o modo portátil só existe no build de release.
        assert!(raiz().is_none());
        assert!(!portatil_ativo());
    }
}
