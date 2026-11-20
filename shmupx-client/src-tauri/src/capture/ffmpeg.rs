use std::io;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;
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

/// Altura de toda gravação de run.
pub const ALTURA_GRAVACAO: u32 = 720;

/// Como capturar a tela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Captura {
    /// Desktop Duplication (`ddagrab`, pela GPU): só o monitor principal, sem perder quadros.
    MonitorPrincipal,
    /// GDI (`gdigrab`): reserva quando o `ddagrab` não inicia (área de trabalho remota, driver
    /// antigo). Pega todos os monitores juntos e é lento em telas grandes.
    AreaDeTrabalho,
}

/// Configuração de uma gravação local (e stream RTMP opcional).
#[derive(Debug, Clone)]
pub struct RecordingConfig {
    pub output_path: PathBuf,
    /// Se presente, também transmite via RTMP (ex.: YouTube Live).
    pub rtmp_url: Option<String>,
    /// Taxa de quadros da captura de tela (gdigrab). 60 fps é importante para a leitura
    /// correta de HUD/placar em shmups rápidos (ver docs/adr/002).
    pub framerate: u32,
    /// Altura do vídeo gravado. Sempre 720p: é a resolução de referência dos scripts de validação
    /// (`reference_height`), e um vídeo maior só deixa a análise mais lenta sem ler melhor o HUD.
    pub altura: u32,
    pub captura: Captura,
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
            altura: ALTURA_GRAVACAO,
            captura: Captura::MonitorPrincipal,
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

static EMPACOTADO: OnceLock<PathBuf> = OnceLock::new();

/// Registra o FFmpeg que vem com o app (`ffmpeg/ffmpeg.exe` nos recursos), se existir. Em
/// desenvolvimento ele não existe e vale o do PATH.
pub fn usar_empacotado(recursos: Option<PathBuf>) {
    if let Some(exe) = recursos
        .map(|dir| dir.join("ffmpeg").join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }))
        .filter(|p| p.is_file())
    {
        let _ = EMPACOTADO.set(exe);
    }
}

/// Programa a executar: o empacotado ou, sem ele, `ffmpeg` do PATH.
pub fn programa() -> PathBuf {
    EMPACOTADO
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from("ffmpeg"))
}

/// Verifica se o FFmpeg (empacotado ou do PATH) executa.
pub fn is_ffmpeg_available() -> bool {
    std::process::Command::new(programa())
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
    let mut cmd = Command::new(programa());
    cmd.arg("-y");

    // Vídeo: a tela, reduzida para a altura da gravação (ver `filtro_de_video`).
    // Linux (X11/XWayland): a tela do `DISPLAY` atual pelo `x11grab`.
    #[cfg(not(windows))]
    {
        let tela = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
        cmd.args(["-f", "x11grab", "-framerate", &config.framerate.to_string(), "-i", &tela]);
        cmd.args(["-vf", &format!("scale=-2:{}:flags=area,format=yuv420p", config.altura)]);
    }
    #[cfg(windows)]
    match config.captura {
        Captura::MonitorPrincipal => {
            cmd.args(["-filter_complex", &filtro_de_video(config)]);
        }
        Captura::AreaDeTrabalho => {
            cmd.args([
                "-f",
                "gdigrab",
                "-framerate",
                &config.framerate.to_string(),
                "-i",
                "desktop",
            ]);
            cmd.args(["-vf", &filtro_de_video(config)]);
        }
    }
    // Taxa constante: o motor de validação calcula o tempo de cada quadro por índice / fps.
    // Sem mudança na tela, o ddagrab não gera quadro; aqui o anterior é repetido.
    cmd.args(["-fps_mode", "cfr", "-r", &config.framerate.to_string()]);

    // Áudio: mixer do sistema via DirectShow — efeitos sonoros do jogo sem microfone externo.
    #[cfg(windows)]
    if let Some(audio_device) = &config.audio_device {
        cmd.args(["-f", "dshow", "-i", &format!("audio={audio_device}")]);
    }

    cmd.args(["-c:v", "libx264", "-preset", &config.video_preset]);

    if cfg!(windows) && config.audio_device.is_some() {
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

/// Filtro de vídeo da gravação: reduz para `altura` mantendo a proporção (largura par, exigida pelo
/// yuv420p). `area` é a melhor interpolação para reduzir texto pequeno (placar). O ddagrab entrega
/// quadros na GPU: `hwdownload` os traz para a CPU (a redução na GPU não funciona em todo driver).
#[cfg_attr(not(windows), allow(dead_code))]
fn filtro_de_video(config: &RecordingConfig) -> String {
    let reduzir = format!("scale=-2:{}:flags=area,format=yuv420p", config.altura);
    match config.captura {
        Captura::MonitorPrincipal => {
            format!(
                "ddagrab=framerate={},hwdownload,format=bgra,{reduzir}",
                config.framerate
            )
        }
        Captura::AreaDeTrabalho => reduzir,
    }
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
    Err(io::Error::other(format!(
        "a captura de tela não iniciou: {}",
        ultima_linha(&log)
    )))
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
        assert_eq!(config.altura, 720);
        assert_eq!(config.captura, Captura::MonitorPrincipal);
        assert_eq!(
            filtro_de_video(&config),
            "ddagrab=framerate=60,hwdownload,format=bgra,scale=-2:720:flags=area,format=yuv420p"
        );
        let reserva = RecordingConfig {
            captura: Captura::AreaDeTrabalho,
            ..config.clone()
        };
        assert_eq!(
            filtro_de_video(&reserva),
            "scale=-2:720:flags=area,format=yuv420p"
        );
        assert_eq!(
            config.log_path(),
            PathBuf::from("C:/dados/shmupx_run_1.ffmpeg.log")
        );
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
