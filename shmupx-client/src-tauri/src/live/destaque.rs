//! Destaque no canal oficial da ShmupX: a run Ranked com live que entra no top 3 do ranking tem
//! o vídeo da live enviado ao canal oficial.
//!
//! A live não grava nada no PC. O backend calcula a posição ao receber o resultado da run e, no
//! top 3, abre o envio do vídeo no canal oficial (título com o nome do jogador, o placar e a
//! posição) e devolve só o endereço desse envio. Daqui: depois que a live termina, o vídeo dela é
//! baixado do YouTube (`ytdlp`) para uma pasta temporária, a tela de espera do começo é cortada e
//! o vídeo vai ao YouTube em partes (upload retomável: uma queda de rede retoma de onde parou). O
//! backend registra o vídeo publicado e a pasta temporária é apagada.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use super::{ytdlp, LiveState};
use crate::capture::ffmpeg;
use crate::session::SessionState;

pub const EVENTO: &str = "run://destaque";
/// Parte de cada envio: múltiplo de 256 KiB, como o YouTube exige.
const PARTE: u64 = 32 * 256 * 1024;
const TENTATIVAS: u32 = 6;
/// Quanto esperar a live terminar antes de desistir.
const ESPERA_DA_LIVE: Duration = Duration::from_secs(10 * 60);

/// Destaque aberto pelo backend no envio do resultado (`POST /api/runs`).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Destaque {
    pub posicao: usize,
    pub score: i64,
    pub upload_url: String,
}

#[derive(Debug, Clone, Serialize)]
struct Andamento {
    jogo_id: String,
    run_id: String,
    posicao: usize,
    /// `uploading`, `posted` ou `failed`.
    status: &'static str,
    video_url: Option<String>,
    progresso: Option<f64>,
}

/// A live da run já terminou e o vídeo dela está guardado (ou não houve live).
async fn esperar_a_live(app: &AppHandle) {
    let live = app.state::<LiveState>().inner().clone();
    let inicio = tokio::time::Instant::now();
    while live.ativa() && inicio.elapsed() < ESPERA_DA_LIVE {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Run fora do top 3 (ou sem destaque): esquece a live dela (o vídeo continua no canal do jogador).
pub fn descartar(app: &AppHandle, run_id: &str) {
    let (app, run_id) = (app.clone(), run_id.to_string());
    tauri::async_runtime::spawn(async move {
        esperar_a_live(&app).await;
        let _ = app.state::<LiveState>().tomar_live(&run_id);
    });
}

/// Pasta temporária do destaque de uma run (fora da pasta das gravações), apagada no fim.
fn pasta_temporaria(app: &AppHandle, run_id: &str) -> PathBuf {
    let base = app.path().app_cache_dir().unwrap_or_else(|_| std::env::temp_dir());
    let nome: String = run_id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    base.join("destaques").join(nome)
}

/// Run do top 3 com live: baixa o vídeo da live, corta a tela de espera e envia ao canal oficial,
/// em segundo plano.
pub fn enviar(app: &AppHandle, jogo_id: &str, run_id: &str, destaque: Destaque) {
    let (app, jogo_id, run_id) = (app.clone(), jogo_id.to_string(), run_id.to_string());
    tauri::async_runtime::spawn(async move {
        let avisar = |status, video_url: Option<String>, progresso: Option<f64>| {
            let _ = app.emit(EVENTO, Andamento { jogo_id: jogo_id.clone(), run_id: run_id.clone(), posicao: destaque.posicao, status, video_url, progresso });
        };
        esperar_a_live(&app).await;
        let Some(live) = app.state::<LiveState>().tomar_live(&run_id) else {
            log::warn!("destaque: run {run_id} sem live registrada neste PC");
            avisar("failed", None, None);
            return;
        };
        avisar("uploading", None, Some(0.0));
        let pasta = pasta_temporaria(&app, &run_id);
        let resultado = async {
            let baixado = ytdlp::baixar(&live.video_id, &pasta).await?;
            let cortado = cortar_espera(&baixado, live.corte_s, live.fim_s).await;
            let arquivo = cortado.as_deref().unwrap_or(&baixado);
            let video_id = enviar_arquivo(&destaque.upload_url, arquivo, |p| avisar("uploading", None, Some(p))).await?;
            registrar(&app, &run_id, &video_id).await?;
            Ok::<String, String>(video_id)
        }
        .await;
        let _ = tokio::fs::remove_dir_all(&pasta).await;
        match resultado {
            Ok(video_id) => {
                log::info!("destaque: run {run_id} ({}º lugar) publicada no canal da ShmupX: {video_id}", destaque.posicao);
                avisar("posted", Some(format!("https://www.youtube.com/watch?v={video_id}")), Some(1.0));
            }
            Err(e) => {
                // A live continua no canal do jogador.
                log::warn!("destaque: run {run_id} não foi enviada ao canal da ShmupX: {e}");
                avisar("failed", None, None);
            }
        }
    });
}

/// Tira a tela de espera do começo (sem recodificar: corta no quadro-chave anterior) e, numa live
/// com outras runs depois desta, o que veio depois dela (`fim_s`). `None`: sem nada a cortar, ou o
/// corte falhou (vai o vídeo inteiro).
async fn cortar_espera(video: &Path, corte_s: f64, fim_s: Option<f64>) -> Option<PathBuf> {
    let fim_s = fim_s.filter(|fim| *fim > corte_s + 1.0);
    if corte_s < 1.0 && fim_s.is_none() {
        return None;
    }
    let saida = video.with_extension("destaque.mp4");
    let mut cmd = tokio::process::Command::new(ffmpeg::programa());
    cmd.args(["-hide_banner", "-loglevel", "error", "-y", "-ss", &format!("{corte_s:.2}")]);
    if let Some(fim) = fim_s {
        cmd.args(["-t", &format!("{:.2}", fim - corte_s)]);
    }
    cmd.arg("-i")
        .arg(video)
        .args(["-c", "copy", "-movflags", "+faststart"])
        .arg(&saida)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let ok = matches!(tokio::time::timeout(Duration::from_secs(300), cmd.status()).await, Ok(Ok(s)) if s.success());
    if ok && saida.is_file() {
        Some(saida)
    } else {
        log::warn!("destaque: não foi possível cortar a tela de espera; vai o vídeo inteiro");
        let _ = tokio::fs::remove_file(&saida).await;
        None
    }
}

/// Até onde o YouTube já recebeu (`Range: bytes=0-N` de uma resposta 308).
fn recebido_ate(resposta: &reqwest::Response) -> Option<u64> {
    let range = resposta.headers().get(reqwest::header::RANGE)?.to_str().ok()?;
    range.rsplit('-').next()?.trim().parse::<u64>().ok().map(|n| n + 1)
}

/// Pergunta ao YouTube quanto do envio já chegou (depois de uma queda de rede).
async fn consultar(http: &reqwest::Client, url: &str, total: u64) -> Result<Option<u64>, String> {
    let resposta = http
        .put(url)
        .header(reqwest::header::CONTENT_LENGTH, 0)
        .header(reqwest::header::CONTENT_RANGE, format!("bytes */{total}"))
        .body(Vec::<u8>::new())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    match resposta.status().as_u16() {
        308 => Ok(Some(recebido_ate(&resposta).unwrap_or(0))),
        200 | 201 => Ok(None),
        s => Err(format!("o YouTube respondeu HTTP {s}")),
    }
}

/// Envia o arquivo ao endereço do envio retomável, em partes; devolve o id do vídeo publicado.
async fn enviar_arquivo(url: &str, arquivo: &Path, progresso: impl Fn(f64)) -> Result<String, String> {
    let http = reqwest::Client::builder().timeout(Duration::from_secs(300)).build().map_err(|e| e.to_string())?;
    let mut file = tokio::fs::File::open(arquivo).await.map_err(|e| format!("vídeo da live ilegível: {e}"))?;
    let total = file.metadata().await.map_err(|e| e.to_string())?.len();
    if total == 0 {
        return Err("vídeo da live vazio".into());
    }
    let mut enviado = 0u64;
    let mut falhas = 0u32;
    let mut parte = Vec::with_capacity(PARTE as usize);
    loop {
        let tamanho = PARTE.min(total - enviado);
        parte.resize(tamanho as usize, 0);
        file.seek(std::io::SeekFrom::Start(enviado)).await.map_err(|e| e.to_string())?;
        file.read_exact(&mut parte).await.map_err(|e| e.to_string())?;
        let resposta = http
            .put(url)
            .header(reqwest::header::CONTENT_LENGTH, tamanho)
            .header(reqwest::header::CONTENT_RANGE, format!("bytes {enviado}-{}/{total}", enviado + tamanho - 1))
            .body(parte.clone())
            .send()
            .await;
        let erro = match resposta {
            Ok(r) if r.status().as_u16() == 308 => {
                enviado = recebido_ate(&r).unwrap_or(enviado + tamanho);
                falhas = 0;
                progresso(enviado as f64 / total as f64);
                continue;
            }
            Ok(r) if r.status().is_success() => {
                let video: serde_json::Value = r.json().await.map_err(|e| format!("resposta do YouTube inesperada: {e}"))?;
                return video["id"].as_str().map(str::to_string).ok_or_else(|| "o YouTube não devolveu o id do vídeo".to_string());
            }
            // Envio expirado ou recusado: não adianta repetir.
            Ok(r) if r.status().is_client_error() => return Err(format!("o YouTube recusou o envio (HTTP {})", r.status())),
            Ok(r) => format!("HTTP {}", r.status()),
            Err(e) => e.to_string(),
        };
        falhas += 1;
        if falhas > TENTATIVAS {
            return Err(format!("o envio ao YouTube falhou: {erro}"));
        }
        tokio::time::sleep(Duration::from_secs(2u64.pow(falhas.min(5)))).await;
        match consultar(&http, url, total).await {
            Ok(Some(ate)) => enviado = ate,
            Ok(None) => return Err("o envio terminou sem o id do vídeo".into()),
            Err(e) => log::warn!("destaque: consulta do envio falhou: {e}"),
        }
    }
}

/// Avisa o backend do vídeo publicado (o endereço do envio deixa de valer).
async fn registrar(app: &AppHandle, run_id: &str, video_id: &str) -> Result<(), String> {
    let token = app.state::<SessionState>().snapshot().token.ok_or("sem sessão para registrar o destaque")?;
    crate::net::backend::chamar_json(
        &token,
        reqwest::Method::POST,
        "/api/runs/destaque",
        Some(serde_json::json!({ "run_id": run_id, "video_id": video_id })),
    )
    .await
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    #[test]
    fn parte_e_multiplo_de_256_kib() {
        assert_eq!(super::PARTE % (256 * 1024), 0);
    }
}
