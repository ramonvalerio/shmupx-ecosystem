//! Bounded context `capture`: gravação via FFmpeg e timer seguro da run.
//! O timer é a fonte da verdade antifraude (docs/specs/002-economia-fichas-e-ranking.md).

pub mod ffmpeg;
pub mod run_timer;

pub use ffmpeg::{FfmpegSession, RecordingConfig};
pub use run_timer::{RunTimer, RunTimerState};
