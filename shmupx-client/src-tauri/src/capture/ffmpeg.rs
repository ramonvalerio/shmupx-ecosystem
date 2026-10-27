use std::io;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};

/// Nome do dispositivo DirectShow usado por padrão para capturar o áudio do *sistema*
/// (mixer de saída, não microfone). Depende de um driver de loopback instalado na máquina
/// do jogador (ex.: "virtual-audio-capturer"/VB-Cable/Stereo Mix habilitado).
///
/// TODO(TASKS.md 3.1b): detectar os dispositivos `dshow` disponíveis em runtime (via
/// `ffmpeg -list_devices true -f dshow -i dummy`) e permitir override nas configurações,
/// em vez de fixar este nome.
pub const DEFAULT_AUDIO_DEVICE: &str = "virtual-audio-capturer";

/// Configuração de uma gravação local (e stream RTMP opcional).
#[derive(Debug, Clone)]
pub struct RecordingConfig {
    pub output_path: PathBuf,
    /// Se presente, também transmite via RTMP (ex.: YouTube Live).
    pub rtmp_url: Option<String>,
    /// Taxa de quadros da captura de tela (gdigrab). 60 fps é importante para a leitura
    /// correta de HUD/placar em shmups rápidos (ver docs/adr/002).
    pub framerate: u32,
    /// Preset do `libx264`; `ultrafast`/`superfast` priorizam CPU baixa sobre tamanho do arquivo,
    /// já que a codificação roda em paralelo com o jogo.
    pub video_preset: String,
    /// Nome do dispositivo de áudio DirectShow (`dshow`) a capturar. `None` grava sem áudio.
    pub audio_device: Option<String>,
}

impl RecordingConfig {
    /// Configuração padrão de uma run: 60 fps, preset `ultrafast`, **sem áudio**. A validação
    /// lê só a imagem, e o dispositivo de loopback ([`DEFAULT_AUDIO_DEVICE`]) raramente existe:
    /// pedir um áudio ausente faz o FFmpeg abortar no início e a run fica sem vídeo. O áudio volta
    /// como opção quando houver transmissão (com o dispositivo detectado, não fixo).
    pub fn new(output_path: PathBuf) -> Self {
        Self {
            output_path,
            rtmp_url: None,
            framerate: 60,
            video_preset: "ultrafast".to_string(),
            audio_device: None,
        }
    }

    /// Log do FFmpeg ao lado do vídeo (`run.mp4` -> `run.ffmpeg.log`), para diagnosticar falhas.
    pub fn log_path(&self) -> PathBuf {
        self.output_path.with_extension("ffmpeg.log")
    }
}

/// Sessão de FFmpeg em segundo plano. Corpo pendente (ver TASKS.md 3.5–3.7).
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

/// Inicia o processo filho do FFmpeg em segundo plano: captura a tela inteira do Windows
/// via GDI (`gdigrab`) a taxa fixa, mixa com o áudio do sistema via DirectShow (`dshow`,
/// sem depender de microfone) e codifica em tempo real com `libx264`/`aac`.
pub fn spawn_ffmpeg(config: &RecordingConfig) -> io::Result<Child> {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y");

    // Vídeo: tela inteira do Windows via GDI, a uma taxa de quadros fixa (60 fps por padrão).
    cmd.args([
        "-f",
        "gdigrab",
        "-framerate",
        &config.framerate.to_string(),
        "-i",
        "desktop",
    ]);

    // Áudio: mixer do sistema via DirectShow — efeitos sonoros do jogo sem microfone externo.
    if let Some(audio_device) = &config.audio_device {
        cmd.args(["-f", "dshow", "-i", &format!("audio={audio_device}")]);
    }

    cmd.args([
        "-c:v",
        "libx264",
        "-preset",
        &config.video_preset,
        "-pix_fmt",
        "yuv420p",
    ]);

    if config.audio_device.is_some() {
        cmd.args(["-c:a", "aac"]);
    }

    // MP4 fragmentado (moov no início, não no fim): mesmo se o processo for encerrado
    // abruptamente pelo corte do timer seguro (kill, não `graceful_stop`), o arquivo
    // gravado até aquele ponto permanece legível em vez de ficar com índice corrompido
    // (ver docs/specs/002, seção 2: "só o trecho gravado até então é elegível").
    cmd.args(["-movflags", "+frag_keyframe+empty_moov"]);

    cmd.arg(&config.output_path);

    if let Some(rtmp_url) = &config.rtmp_url {
        cmd.args(["-f", "flv", rtmp_url]);
    }

    // O stderr vai para um arquivo: sem ele, uma falha do FFmpeg (ex.: dispositivo ausente) some
    // e a run parece gravar sem gerar vídeo.
    let log = std::fs::File::create(config.log_path())?;
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::from(log));
    cmd.kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    cmd.spawn()
}

/// Tempo que o FFmpeg precisa sobreviver para a captura contar como iniciada.
const CONFIRMACAO_INICIO: Duration = Duration::from_millis(1500);

/// Confirma que a captura começou: o FFmpeg aborta logo no início quando algo está errado
/// (dispositivo, codec, permissão). Se ele já saiu, devolve a última linha do log como motivo.
pub async fn confirmar_inicio(child: &mut Child, config: &RecordingConfig) -> io::Result<()> {
    tokio::time::sleep(CONFIRMACAO_INICIO).await;
    if child.try_wait()?.is_none() {
        return Ok(());
    }
    let log = std::fs::read_to_string(config.log_path()).unwrap_or_default();
    Err(io::Error::other(format!("a captura de tela não iniciou: {}", ultima_linha(&log))))
}

/// Última linha útil do log do FFmpeg (as anteriores costumam ser banner e configuração).
pub fn ultima_linha(log: &str) -> String {
    log.lines()
        .rev()
        .map(str::trim)
        .find(|linha| !linha.is_empty())
        .map(|linha| linha.chars().take(300).collect())
        .unwrap_or_else(|| "sem detalhe no log do FFmpeg".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_padrao_grava_sem_audio_e_log_fica_ao_lado_do_video() {
        let config = RecordingConfig::new(PathBuf::from("C:/dados/shmupx_run_1.mp4"));
        assert!(config.audio_device.is_none());
        assert_eq!(config.log_path(), PathBuf::from("C:/dados/shmupx_run_1.ffmpeg.log"));
    }

    #[test]
    fn ultima_linha_do_log_explica_a_falha() {
        let log = "ffmpeg version 8.0\n  built with gcc\nError opening input file audio=virtual-audio-capturer.\nError opening input files: I/O error\n\n";
        assert_eq!(ultima_linha(log), "Error opening input files: I/O error");
        assert_eq!(ultima_linha(""), "sem detalhe no log do FFmpeg");
    }
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
