use std::path::PathBuf;

/// Configuração de uma gravação local (e stream RTMP opcional).
#[derive(Debug, Clone)]
pub struct RecordingConfig {
    pub output_path: PathBuf,
    /// Se presente, também transmite via RTMP (ex.: YouTube Live).
    pub rtmp_url: Option<String>,
}

/// Sessão de FFmpeg em segundo plano. Corpo pendente (ver TASKS.md).
pub struct FfmpegSession {
    pub config: RecordingConfig,
}

impl FfmpegSession {
    pub fn new(config: RecordingConfig) -> Self {
        Self { config }
    }
}
