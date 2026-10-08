use std::io;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};

/// Configuração de uma gravação local (e stream RTMP opcional).
#[derive(Debug, Clone)]
pub struct RecordingConfig {
    pub output_path: PathBuf,
    /// Se presente, também transmite via RTMP (ex.: YouTube Live).
    pub rtmp_url: Option<String>,
}

/// Sessão de FFmpeg em segundo plano. Corpo pendente (ver TASKS.md 3.2–3.5).
pub struct FfmpegSession {
    pub config: RecordingConfig,
}

impl FfmpegSession {
    pub fn new(config: RecordingConfig) -> Self {
        Self { config }
    }
}

/// Verifica se o binário `ffmpeg` está acessível no PATH do sistema operacional.
pub fn is_ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Inicia o processo filho do FFmpeg em segundo plano, de forma assíncrona.
///
/// A linha de comando de captura real (gdigrab/ddagrab + WASAPI) é definida em TASKS.md 3.2;
/// este esqueleto grava um padrão de teste (`testsrc`) apenas para validar o ciclo de vida
/// do processo (spawn, PID, corte por timer, parada graciosa).
pub fn spawn_ffmpeg(config: &RecordingConfig) -> io::Result<Child> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-y",
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=640x360:rate=30",
    ]);
    cmd.arg(&config.output_path);

    if let Some(rtmp_url) = &config.rtmp_url {
        cmd.args(["-f", "flv", rtmp_url]);
    }

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.kill_on_drop(true);

    cmd.spawn()
}

/// Parada graciosa: envia `q` ao stdin do FFmpeg (fecha o arquivo corretamente) e aguarda
/// a saída do processo; se não finalizar dentro do timeout, força o encerramento (`kill`).
pub async fn graceful_stop(child: &mut Child) -> io::Result<()> {
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(b"q\n").await;
        let _ = stdin.shutdown().await;
    }

    match tokio::time::timeout(Duration::from_secs(3), child.wait()).await {
        Ok(_) => Ok(()),
        Err(_) => child.kill().await,
    }
}
