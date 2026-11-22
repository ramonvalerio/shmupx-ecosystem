//! Runs cuja análise ou envio falhou ("Não enviada"). Ficam em `runs-pendentes.json`, nos dados
//! do app, com o vídeo preservado: o jogador pode analisar de novo depois (regra nova do
//! validador, rede de volta, sessão renovada) sem perder a ficha. O servidor aceita o resultado
//! de uma run encerrada a qualquer momento (`processing`). Uma run enviada sai da lista.
use std::path::PathBuf;
use std::sync::Mutex;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunPendente {
    pub run_id: String,
    pub jogo_id: String,
    pub video_path: PathBuf,
    /// Dono da run (conta do Firebase): outra conta no mesmo PC não vê nem reenvia.
    pub uid: Option<String>,
    /// Início e fim da gravação (ms desde a época Unix).
    pub iniciada_em: u64,
    pub encerrada_em: Option<u64>,
    pub erro: String,
}

/// Leituras e escritas do arquivo, uma de cada vez.
static ARQUIVO: Mutex<()> = Mutex::new(());

fn caminho(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|pasta| pasta.join("runs-pendentes.json"))
}

fn ler(app: &AppHandle) -> Vec<RunPendente> {
    caminho(app)
        .and_then(|arquivo| std::fs::read(arquivo).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn gravar(app: &AppHandle, lista: &[RunPendente]) {
    let Some(arquivo) = caminho(app) else { return };
    if let Some(pasta) = arquivo.parent() {
        let _ = std::fs::create_dir_all(pasta);
    }
    match serde_json::to_vec_pretty(lista) {
        Ok(bytes) => {
            if let Err(erro) = std::fs::write(&arquivo, bytes) {
                log::warn!("runs pendentes: falha ao gravar {}: {erro}", arquivo.display());
            }
        }
        Err(erro) => log::warn!("runs pendentes: {erro}"),
    }
}

/// `uid` do ID token do Firebase (só lê o conteúdo; quem confere a assinatura é o backend).
pub fn uid_do_token(token: &str) -> Option<String> {
    let corpo = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(corpo.trim_end_matches('=')).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    claims.get("user_id").or_else(|| claims.get("sub"))?.as_str().map(str::to_string)
}

/// Início da gravação pelo nome do vídeo (`shmupx_run_{ms}.mp4`).
pub fn inicio_pelo_video(video: &std::path::Path) -> u64 {
    video
        .file_stem()
        .and_then(|nome| nome.to_str())
        .and_then(|nome| nome.strip_prefix("shmupx_run_"))
        .and_then(|ms| ms.parse().ok())
        .unwrap_or_default()
}

/// Fim da gravação: a última escrita do vídeo.
pub fn fim_pelo_video(video: &std::path::Path) -> Option<u64> {
    let modificado = std::fs::metadata(video).ok()?.modified().ok()?;
    modificado
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as u64)
}

/// Guarda (ou atualiza) a run que falhou.
pub fn guardar(app: &AppHandle, run: RunPendente) {
    let _vez = ARQUIVO.lock().unwrap_or_else(|p| p.into_inner());
    let mut lista = ler(app);
    lista.retain(|r| r.run_id != run.run_id);
    lista.insert(0, run);
    gravar(app, &lista);
}

pub fn remover(app: &AppHandle, run_id: &str) {
    let _vez = ARQUIVO.lock().unwrap_or_else(|p| p.into_inner());
    let mut lista = ler(app);
    let antes = lista.len();
    lista.retain(|r| r.run_id != run_id);
    if lista.len() != antes {
        gravar(app, &lista);
    }
}

pub fn buscar(app: &AppHandle, run_id: &str) -> Option<RunPendente> {
    let _vez = ARQUIVO.lock().unwrap_or_else(|p| p.into_inner());
    ler(app).into_iter().find(|r| r.run_id == run_id)
}

/// Runs pendentes da conta, só as que ainda têm o vídeo.
pub fn listar(app: &AppHandle, uid: Option<&str>) -> Vec<RunPendente> {
    let _vez = ARQUIVO.lock().unwrap_or_else(|p| p.into_inner());
    ler(app)
        .into_iter()
        .filter(|r| r.video_path.is_file())
        .filter(|r| match (&r.uid, uid) {
            (Some(dono), Some(atual)) => dono == atual,
            _ => true,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_o_uid_do_token() {
        let corpo = URL_SAFE_NO_PAD.encode(br#"{"user_id":"abc123","sub":"abc123"}"#);
        assert_eq!(uid_do_token(&format!("x.{corpo}.y")).as_deref(), Some("abc123"));
        assert_eq!(uid_do_token("invalido"), None);
    }

    #[test]
    fn inicio_vem_do_nome_do_video() {
        assert_eq!(inicio_pelo_video(std::path::Path::new("C:/r/shmupx_run_1795376028095.mp4")), 1_795_376_028_095);
        assert_eq!(inicio_pelo_video(std::path::Path::new("C:/r/outro.mp4")), 0);
    }
}
