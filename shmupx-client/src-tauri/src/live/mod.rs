//! START LIVE: transmissão ao vivo no YouTube do jogador (spec 033/036 do Client antigo).
//!
//! Fluxo: acha a janela do jogo da sala (ou abre o jogo pela Steam e espera) → o backend cria a
//! live (não listada, ou pública se o jogador marcar) e devolve o endereço RTMP → só a janela do
//! jogo é capturada (`janela`: quadro fixo 1280×720, o jogo encaixado sem distorcer, inclusive em
//! tela cheia) e vai ao FFmpeg com o som do sistema → quando o YouTube acusa o vídeo chegando, a
//! live vai ao ar → termina no END LIVE, se o jogo fechar, se o FFmpeg cair ou em 45 min.
//!
//! A live não grava MP4 (D-03301) e não convive com uma run gravando: as duas disputam a captura.
//! O endereço RTMP leva a chave da transmissão: nunca vai para log (FFmpeg em `-loglevel error`).

mod audio;
pub mod commands;
#[cfg(windows)]
pub mod janela;
#[cfg(not(windows))]
#[path = "janela_outros.rs"]
pub mod janela;

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
    /// Captura da janela do jogo (para ao ser descartada).
    captura: Option<janela::CapturaJanela>,
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

/// FFmpeg da live a partir da janela do jogo: vídeo BGRA 1280×720 a 60 quadros/s pelo stdin
/// (entrada 0) e o áudio `f32le` por uma conexão TCP local (entrada 1, o FFmpeg escuta).
fn spawn_ffmpeg_janela(ingest: &str, taxa_audio: u32, porta_audio: u16, log: &PathBuf) -> std::io::Result<Child> {
    let mut cmd = Command::new(ffmpeg::programa());
    cmd.args(["-hide_banner", "-loglevel", "error", "-y"]);
    let tamanho = format!("{}x{}", janela::LARGURA, janela::ALTURA);
    let fps = janela::QUADROS_POR_SEGUNDO.to_string();
    // Formatos conhecidos: sem análise das entradas. Analisar segurava o FFmpeg ~4 s antes de
    // começar, o som acumulava nesse tempo e a live saía com o áudio atrasado.
    let sem_analise = ["-probesize", "32", "-analyzeduration", "0"];
    cmd.args(sem_analise);
    cmd.args(["-thread_queue_size", "64", "-f", "rawvideo", "-pix_fmt", "bgra", "-s", &tamanho, "-framerate", &fps, "-i", "pipe:0"]);
    cmd.args(sem_analise);
    cmd.args(["-thread_queue_size", "1024", "-f", "f32le", "-ar", &taxa_audio.to_string(), "-ac", "2", "-i", &format!("tcp://127.0.0.1:{porta_audio}?listen=1")]);
    cmd.args(["-map", "0:v", "-map", "1:a", "-vf", "format=yuv420p"]);
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

/// Porta livre em 127.0.0.1 (para o FFmpeg escutar o áudio).
fn porta_livre() -> Result<u16, String> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| format!("sem porta local para o áudio: {e}"))
}

/// Liga o sinal de parada da live (`parar_audio`) ao de cada escritor.
fn ligar_parada(da_live: Arc<AtomicBool>, do_escritor: Arc<AtomicBool>) {
    tokio::spawn(async move {
        while !da_live.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        do_escritor.store(true, Ordering::SeqCst);
    });
}

/// Envio a partir da janela do jogo: captura, FFmpeg, escritor de vídeo e de áudio.
async fn abrir_envio_janela(ingest: &str, log: &PathBuf, parar_audio: Arc<AtomicBool>, tela: janela::Tela, pid: u32) -> Result<(Child, bool), String> {
    let loopback = match audio::iniciar(pid) {
        Ok(l) => Some(l),
        Err(e) => {
            log::warn!("live: sem som do sistema ({e}); a live sai com silêncio");
            None
        }
    };
    let silenciosa = loopback.is_none();
    let taxa = loopback.as_ref().map_or(audio::TAXA_PADRAO, |l| l.taxa);
    let porta = porta_livre()?;
    let mut child = spawn_ffmpeg_janela(ingest, taxa, porta, log).map_err(|e| format!("FFmpeg indisponível: {e}"))?;
    let stdin = child.stdin.take().ok_or("FFmpeg sem entrada de vídeo")?;
    #[cfg(windows)]
    let stdin = std::fs::File::from(stdin.into_owned_handle().map_err(|e| format!("FFmpeg sem entrada de vídeo: {e}"))?);
    #[cfg(unix)]
    let stdin = std::fs::File::from(stdin.into_owned_fd().map_err(|e| format!("FFmpeg sem entrada de vídeo: {e}"))?);
    let parar = Arc::new(AtomicBool::new(false));
    janela::alimentar(stdin, tela, parar.clone());
    // O FFmpeg abre a entrada de áudio depois de ler o vídeo: tenta conectar por até 10 s.
    let conexao = async {
        for _ in 0..100 {
            if let Ok(conexao) = tokio::net::TcpStream::connect(("127.0.0.1", porta)).await {
                return Some(conexao);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        None
    }
    .await;
    let Some(conexao) = conexao else {
        parar.store(true, Ordering::SeqCst);
        let _ = child.kill().await;
        return Err(format!("o envio do vídeo não iniciou: {}", motivo_do_log(log)));
    };
    tokio::spawn(audio::alimentar(conexao, loopback, parar.clone()));
    tokio::time::sleep(Duration::from_millis(1500)).await;
    if child.try_wait().ok().flatten().is_some() {
        parar.store(true, Ordering::SeqCst);
        return Err(format!("o envio do vídeo não iniciou: {}", motivo_do_log(log)));
    }
    ligar_parada(parar_audio, parar);
    Ok((child, silenciosa))
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

/// Leva a live do início ao ar. Erros viram `failed` com a mensagem; cancelar volta a `idle`.
pub async fn conduzir(app: AppHandle, live: LiveState, token: String, jogo_id: String, titulo: String, publica: bool, steam_app_id: Option<u32>, log: PathBuf) {
    let resultado = tokio::select! {
        r = colocar_no_ar(&app, &live, &token, &jogo_id, &titulo, publica, steam_app_id, &log) => r,
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

/// PIDs do jogo da sala: pela pasta de instalação da Steam ou, sem Steam, o ShmupX Emulator.
fn pids_do_jogo(steam_app_id: Option<u32>) -> Vec<u32> {
    match steam_app_id.filter(|id| *id > 0) {
        Some(app_id) => crate::steam::biblioteca::pasta_do_jogo(app_id).map(|p| crate::steam::detector::processos_na_pasta(&p)).unwrap_or_default(),
        None => crate::steam::detector::processos_por_nome("shmupx-engine.exe"),
    }
}

/// Janela do jogo: a que já está aberta ou espera ela aparecer. Na Steam, abre o jogo (até 90 s);
/// no emulador, a interface acabou de abrir o ShmupX Emulator e a janela leva uns segundos (até 60 s), como
/// o `WaitingForGame` do Client antigo.
async fn achar_janela(app: &AppHandle, live: &LiveState, base: &Estado, steam_app_id: Option<u32>) -> Result<janela::Window, String> {
    let procurar = move || {
        let pids = pids_do_jogo(steam_app_id);
        janela::janela_dos_processos(&pids)
    };
    if let Some(janela) = tauri::async_runtime::spawn_blocking(procurar).await.ok().flatten() {
        return Ok(janela);
    }
    live.definir(app, Estado { status: "waiting", ..base.clone() });
    let espera = match steam_app_id.filter(|id| *id > 0) {
        Some(app_id) => {
            crate::steam::commands::launch_steam_uri(app_id).map_err(|e| format!("não foi possível abrir o jogo pela Steam: {e}"))?;
            Duration::from_secs(90)
        }
        None => Duration::from_secs(60),
    };
    let inicio = tokio::time::Instant::now();
    let prazo = inicio + espera;
    while tokio::time::Instant::now() < prazo {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if let Some(janela) = tauri::async_runtime::spawn_blocking(procurar).await.ok().flatten() {
            // Dá um instante para o jogo terminar de montar a janela.
            tokio::time::sleep(Duration::from_secs(2)).await;
            return Ok(tauri::async_runtime::spawn_blocking(procurar).await.ok().flatten().unwrap_or(janela));
        }
        // Emulador que nem chegou a abrir (ROM ruim, emulador fechado): não espera o prazo todo.
        if steam_app_id.filter(|id| *id > 0).is_none() && inicio.elapsed() > Duration::from_secs(15) {
            let vivo = tauri::async_runtime::spawn_blocking(move || !pids_do_jogo(None).is_empty()).await.unwrap_or(false);
            if !vivo {
                return Err("o ShmupX Emulator não abriu o jogo; confira o ROM na sua pasta de ROMs e tente de novo".to_string());
            }
        }
    }
    Err("a janela do jogo não apareceu; abra o jogo e tente de novo".to_string())
}

async fn colocar_no_ar(app: &AppHandle, live: &LiveState, token: &str, jogo_id: &str, titulo: &str, publica: bool, steam_app_id: Option<u32>, log: &PathBuf) -> Result<(), String> {
    let base = Estado { game_id: Some(jogo_id.to_string()), public: publica, ..Default::default() };
    // Fora do Windows ainda não há captura da janela do jogo.
    if cfg!(not(windows)) {
        return Err("a live ainda não está disponível no Linux".to_string());
    }
    // Primeiro o jogo (a live captura só a janela dele), depois a live no YouTube.
    let janela_do_jogo = achar_janela(app, live, &base, steam_app_id).await?;
    let captura = janela::iniciar(janela_do_jogo)?;
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
    *live.corrente.lock().await = Some(Corrente { broadcast_id: Some(broadcast_id.clone()), child: None, parar_audio: parar_audio.clone(), captura: None });

    live.definir(app, Estado { status: "connecting", watch_url: Some(watch.clone()), ..base.clone() });
    let (child, silenciosa) = abrir_envio_janela(&ingest, log, parar_audio, captura.tela.clone(), captura.pid).await?;
    if let Some(c) = live.corrente.lock().await.as_mut() {
        c.child = Some(child);
        c.captura = Some(captura);
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
                break false;
            }
            if guarda.as_ref().and_then(|c| c.captura.as_ref()).is_some_and(|c| c.fechada.load(Ordering::SeqCst)) {
                break true;
            }
        }
    };
    let (motivo_erro, pelo_tempo) = tokio::select! {
        _ = live.cancelar.notified() => (None, false),
        _ = tokio::time::sleep(LIMITE) => (None, true),
        jogo_fechou = caiu => if jogo_fechou { (None, false) } else { (Some(format!("a transmissão caiu: {}", motivo_do_log(&log))), false) },
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
    drop(corrente.captura.take());
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

#[cfg(test)]
mod neste_pc {
    use super::*;

    /// Envio completo para um arquivo local, com um tom tocando:
    /// `cargo test -- --ignored envio_local --nocapture`.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "depende de som e FFmpeg neste PC"]
    async fn envio_local() {
        let pasta = std::env::temp_dir();
        let saida = pasta.join("shmupx_live_teste.flv");
        let log = pasta.join("shmupx_live_teste.log");
        let _ = std::fs::remove_file(&saida);
        let tela: janela::Tela = Arc::new(Mutex::new(vec![128; janela::LARGURA * janela::ALTURA * 4]));
        let tom = std::thread::spawn(|| {
            use rodio::Source;
            let (_s, h) = rodio::OutputStream::try_default().unwrap();
            let sink = rodio::Sink::try_new(&h).unwrap();
            sink.append(rodio::source::SineWave::new(440.0).take_duration(Duration::from_secs(5)).amplify(0.3));
            sink.sleep_until_end();
        });
        let parar = Arc::new(AtomicBool::new(false));
        let (mut child, silenciosa) = abrir_envio_janela(saida.to_str().unwrap(), &log, parar.clone(), tela, std::process::id()).await.unwrap();
        println!("silenciosa: {silenciosa}");
        let t0 = std::time::Instant::now();
        tokio::time::sleep(Duration::from_secs(8)).await;
        println!("no ar por {:?}", t0.elapsed());
        parar.store(true, Ordering::SeqCst);
        let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
        tom.join().unwrap();
        println!("log: {}", std::fs::read_to_string(&log).unwrap_or_default());
        println!("saida: {} bytes", std::fs::metadata(&saida).map(|m| m.len()).unwrap_or(0));
    }
}
