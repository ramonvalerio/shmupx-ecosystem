//! START LIVE: transmissão ao vivo no YouTube do jogador (spec 033/036 do Client antigo).
//!
//! Fluxo: o backend cria a live (não listada, ou pública se o jogador marcar) e devolve o endereço
//! RTMP → o FFmpeg captura o monitor principal (canvas fixo 1280×720, o jogo encaixado sem
//! distorcer) com o som do sistema e envia → quando o YouTube acusa o vídeo chegando, a live vai
//! ao ar → termina no END LIVE, se o FFmpeg cair, ou sozinha em 45 min (como a sessão de gravação).
//!
//! A live não grava MP4 (D-03301) e não convive com uma run gravando: as duas disputam a captura.
//! O endereço RTMP leva a chave da transmissão: nunca vai para log (FFmpeg em `-loglevel error`).

mod audio;
pub mod commands;

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::process::{Child, Command};
use tokio::sync::Notify;

use crate::capture::ffmpeg;

/// Limite da live: a mesma sessão de 45 min da gravação (spec 033 FR-03316).
pub const LIMITE: Duration = Duration::from_secs(45 * 60);
/// Tempo para o YouTube acusar o vídeo chegando (spec 033 FR-03306).
const ATIVACAO: Duration = Duration::from_secs(90);
pub const EVENTO: &str = "live://estado";

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Estado {
    /// `idle`, `preparing`, `connecting`, `live`, `ending` ou `failed`.
    pub status: &'static str,
    pub game_id: Option<String>,
    pub watch_url: Option<String>,
    pub public: bool,
    /// Fim automático (ms desde 1970), enquanto no ar.
    pub ends_at_ms: Option<u64>,
    pub error: Option<String>,
    /// A última live terminou pelo limite de 45 min.
    pub ended_by_time_limit: bool,
    /// Som do sistema indisponível: a live está com trilha de silêncio.
    pub silent: bool,
}

struct Corrente {
    broadcast_id: Option<String>,
    child: Option<Child>,
    parar_audio: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct LiveState {
    estado: Arc<Mutex<Estado>>,
    corrente: Arc<tokio::sync::Mutex<Option<Corrente>>>,
    cancelar: Arc<Notify>,
}

impl Default for LiveState {
    fn default() -> Self {
        Self {
            estado: Arc::new(Mutex::new(Estado { status: "idle", ..Default::default() })),
            corrente: Arc::default(),
            cancelar: Arc::default(),
        }
    }
}

impl LiveState {
    pub fn snapshot(&self) -> Estado {
        self.estado.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Live em andamento (de "preparando" até "encerrando"): bloqueia o início de uma run.
    pub fn ativa(&self) -> bool {
        !matches!(self.snapshot().status, "idle" | "failed")
    }

    fn definir(&self, app: &AppHandle, novo: Estado) {
        *self.estado.lock().unwrap_or_else(|p| p.into_inner()) = novo.clone();
        let _ = app.emit(EVENTO, novo);
    }

    fn alterar(&self, app: &AppHandle, f: impl FnOnce(&mut Estado)) {
        let mut atual = self.snapshot();
        f(&mut atual);
        self.definir(app, atual);
    }
}

fn agora_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Canvas fixo 16:9 da live (spec 036): o monitor é encaixado sem distorcer, com barras pretas.
fn filtro_canvas() -> &'static str {
    "scale=1280:720:force_original_aspect_ratio=decrease:flags=bicubic,pad=1280:720:(ow-iw)/2:(oh-ih)/2,format=yuv420p"
}

/// FFmpeg da live: áudio estéreo `f32le` pelo stdin (entrada 0) e a tela pelo `ddagrab` (GPU) ou,
/// como reserva, pelo `gdigrab`. H.264 + AAC em FLV/RTMP, quadro-chave a cada 2 s (YouTube).
fn spawn_ffmpeg(ingest: &str, taxa_audio: u32, gdi: bool, log: &PathBuf) -> std::io::Result<Child> {
    let mut cmd = Command::new(ffmpeg::programa());
    cmd.args(["-hide_banner", "-loglevel", "error", "-y"]);
    cmd.args(["-thread_queue_size", "1024", "-f", "f32le", "-ar", &taxa_audio.to_string(), "-ac", "2", "-i", "pipe:0"]);
    if gdi {
        cmd.args(["-thread_queue_size", "512", "-f", "gdigrab", "-framerate", "60", "-i", "desktop"]);
        cmd.args(["-filter_complex", &format!("[1:v]{}[v]", filtro_canvas())]);
    } else {
        cmd.args(["-filter_complex", &format!("ddagrab=framerate=60,hwdownload,format=bgra,{}[v]", filtro_canvas())]);
    }
    cmd.args(["-map", "[v]", "-map", "0:a"]);
    cmd.args(["-fps_mode", "cfr", "-r", "60"]);
    cmd.args(["-c:v", "libx264", "-preset", "veryfast", "-tune", "zerolatency"]);
    cmd.args(["-b:v", "4500k", "-maxrate", "4500k", "-bufsize", "9000k", "-g", "120", "-keyint_min", "120", "-sc_threshold", "0"]);
    cmd.args(["-c:a", "aac", "-b:a", "160k", "-ar", "48000"]);
    cmd.args(["-f", "flv", ingest]);
    cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::from(std::fs::File::create(log)?));
    cmd.kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd.spawn()
}

/// Primeira linha útil do log do FFmpeg (motivo de uma falha), sem nada que lembre a chave.
fn motivo_do_log(log: &PathBuf) -> String {
    let texto = std::fs::read_to_string(log).unwrap_or_default();
    let linha = ffmpeg::ultima_linha(&texto);
    if linha.contains("rtmp") || linha.contains("live2") {
        "o envio do vídeo ao YouTube falhou".to_string()
    } else if linha.is_empty() {
        "o FFmpeg encerrou sem motivo registrado".to_string()
    } else {
        linha
    }
}

/// Abre o FFmpeg (ddagrab; reserva gdigrab) com o áudio, e confirma que ficou de pé.
async fn abrir_envio(ingest: &str, log: &PathBuf, parar_audio: Arc<AtomicBool>) -> Result<(Child, bool), String> {
    let loopback = match audio::iniciar() {
        Ok(l) => Some(l),
        Err(e) => {
            log::warn!("live: sem som do sistema ({e}); a live sai com silêncio");
            None
        }
    };
    let silenciosa = loopback.is_none();
    let taxa = loopback.as_ref().map_or(audio::TAXA_PADRAO, |l| l.taxa);
    let mut loopback = loopback;
    for gdi in [false, true] {
        let mut child = spawn_ffmpeg(ingest, taxa, gdi, log).map_err(|e| format!("FFmpeg indisponível: {e}"))?;
        let stdin = child.stdin.take().ok_or("FFmpeg sem entrada de áudio")?;
        let parar = Arc::new(AtomicBool::new(false));
        let alimentador = tokio::spawn(audio::alimentar(stdin, loopback.take(), parar.clone()));
        tokio::time::sleep(Duration::from_millis(2500)).await;
        if child.try_wait().ok().flatten().is_none() {
            // Liga o sinal de parada do áudio ao da live.
            tokio::spawn(async move {
                while !parar_audio.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                parar.store(true, Ordering::SeqCst);
            });
            return Ok((child, silenciosa));
        }
        parar.store(true, Ordering::SeqCst);
        let _ = alimentador.await;
        if gdi {
            return Err(format!("a captura da tela não iniciou: {}", motivo_do_log(log)));
        }
        // O ddagrab falha em área de trabalho remota e drivers antigos: tenta pelo GDI. O loopback
        // foi consumido pela tentativa; a reserva sai com o som de novo, se ele abrir.
        loopback = audio::iniciar().ok();
    }
    unreachable!()
}

/// Leva a live do início ao ar. Erros viram `failed` com a mensagem; cancelar volta a `idle`.
pub async fn conduzir(app: AppHandle, live: LiveState, token: String, jogo_id: String, titulo: String, publica: bool, log: PathBuf) {
    let resultado = tokio::select! {
        r = colocar_no_ar(&app, &live, &token, &jogo_id, &titulo, publica, &log) => r,
        _ = live.cancelar.notified() => Err(String::new()),
    };
    match resultado {
        Ok(()) => vigiar(app, live, token, log).await,
        Err(e) => {
            encerrar_recursos(&live, &token).await;
            live.definir(&app, if e.is_empty() {
                Estado { status: "idle", ..Default::default() }
            } else {
                Estado { status: "failed", game_id: Some(jogo_id), error: Some(e), ..Default::default() }
            });
        }
    }
}

async fn colocar_no_ar(app: &AppHandle, live: &LiveState, token: &str, jogo_id: &str, titulo: &str, publica: bool, log: &PathBuf) -> Result<(), String> {
    let base = Estado { game_id: Some(jogo_id.to_string()), public: publica, ..Default::default() };
    live.definir(app, Estado { status: "preparing", ..base.clone() });
    let criada = crate::net::backend::chamar_json(
        token,
        reqwest::Method::POST,
        "/api/live/start",
        Some(serde_json::json!({ "title": titulo, "public": publica })),
    )
    .await?;
    let texto = |c: &str| criada[c].as_str().map(str::to_string).ok_or_else(|| "resposta inesperada do servidor".to_string());
    let (broadcast_id, stream_id, ingest, watch) = (texto("broadcast_id")?, texto("stream_id")?, texto("ingest_url")?, texto("watch_url")?);
    let parar_audio = Arc::new(AtomicBool::new(false));
    *live.corrente.lock().await = Some(Corrente { broadcast_id: Some(broadcast_id.clone()), child: None, parar_audio: parar_audio.clone() });

    live.definir(app, Estado { status: "connecting", watch_url: Some(watch.clone()), ..base.clone() });
    let (child, silenciosa) = abrir_envio(&ingest, log, parar_audio).await?;
    if let Some(c) = live.corrente.lock().await.as_mut() {
        c.child = Some(child);
    }
    live.alterar(app, |e| e.silent = silenciosa);

    // Espera o YouTube receber o vídeo, a menos que o FFmpeg caia antes.
    let prazo = tokio::time::Instant::now() + ATIVACAO;
    loop {
        if let Some(c) = live.corrente.lock().await.as_mut() {
            if let Some(child) = c.child.as_mut() {
                if child.try_wait().ok().flatten().is_some() {
                    return Err(format!("o envio do vídeo parou: {}", motivo_do_log(log)));
                }
            }
        }
        let situacao = crate::net::backend::chamar_json(token, reqwest::Method::GET, &format!("/api/live/streams/{stream_id}/status"), None).await?;
        if situacao["status"].as_str() == Some("active") {
            break;
        }
        if tokio::time::Instant::now() >= prazo {
            return Err("o YouTube não recebeu o vídeo a tempo; verifique sua conexão e tente de novo".to_string());
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }

    crate::net::backend::chamar_json(
        token,
        reqwest::Method::POST,
        &format!("/api/live/broadcasts/{broadcast_id}/transition"),
        Some(serde_json::json!({ "status": "live" })),
    )
    .await?;
    live.definir(app, Estado {
        status: "live",
        watch_url: Some(watch),
        ends_at_ms: Some(agora_ms() + LIMITE.as_millis() as u64),
        silent: silenciosa,
        ..base
    });
    Ok(())
}

/// No ar: termina no END LIVE, se o FFmpeg cair ou no limite de 45 min.
async fn vigiar(app: AppHandle, live: LiveState, token: String, log: PathBuf) {
    let caiu = async {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let mut guarda = live.corrente.lock().await;
            let vivo = guarda
                .as_mut()
                .and_then(|c| c.child.as_mut())
                .is_some_and(|child| child.try_wait().ok().flatten().is_none());
            if !vivo {
                break;
            }
        }
    };
    let (motivo_erro, pelo_tempo) = tokio::select! {
        _ = live.cancelar.notified() => (None, false),
        _ = tokio::time::sleep(LIMITE) => (None, true),
        _ = caiu => (Some(format!("a transmissão caiu: {}", motivo_do_log(&log))), false),
    };
    live.alterar(&app, |e| e.status = "ending");
    let jogo = live.snapshot().game_id;
    encerrar_recursos(&live, &token).await;
    live.definir(&app, match motivo_erro {
        Some(e) => Estado { status: "failed", game_id: jogo, error: Some(e), ..Default::default() },
        None => Estado { status: "idle", ended_by_time_limit: pelo_tempo, ..Default::default() },
    });
}

/// Para o áudio e o FFmpeg e encerra a live no YouTube (melhor esforço: sem rede, os recursos
/// locais são liberados mesmo assim).
async fn encerrar_recursos(live: &LiveState, token: &str) {
    let Some(mut corrente) = live.corrente.lock().await.take() else { return };
    corrente.parar_audio.store(true, Ordering::SeqCst);
    if let Some(mut child) = corrente.child.take() {
        let _ = child.kill().await;
    }
    if let Some(id) = corrente.broadcast_id.take() {
        let _ = crate::net::backend::chamar_json(
            token,
            reqwest::Method::POST,
            &format!("/api/live/broadcasts/{id}/transition"),
            Some(serde_json::json!({ "status": "complete" })),
        )
        .await;
    }
}
