//! Bounded context `capture`: a run ranked (ficha, timer seguro, gravação dos jogos validados pelo
//! vídeo via FFmpeg, análise e envio).
//! O timer é a fonte da verdade antifraude (docs/specs/002-economia-fichas-e-ranking.md).

pub mod audio;
pub mod commands;
pub mod destino;
pub mod ffmpeg;
pub mod gravacao;
#[cfg(windows)]
pub mod janela_nv12;
pub mod pendentes;
pub mod ranking_local;
pub mod run_timer;
pub mod state;

pub use audio::RecordingState;
pub use commands::{finalizar_corrida_manual, iniciar_corrida_safe};
pub use run_timer::{RunTimer, RunTimerState};
pub use state::{AnalysisQueue, RunState};
