use std::time::Duration;

/// Duração máxima de uma run (limite superior da janela de 45–60 min).
pub const MAX_RUN_DURATION: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunTimerState {
    Idle,
    Running,
    Expired,
    Finished,
}

/// Timer seguro da run, mantido em Rust (não em JS). Corpo pendente (ver TASKS.md).
#[derive(Debug)]
pub struct RunTimer {
    pub state: RunTimerState,
    pub limit: Duration,
}

impl RunTimer {
    pub fn new(limit: Duration) -> Self {
        Self { state: RunTimerState::Idle, limit }
    }
}

impl Default for RunTimer {
    fn default() -> Self {
        Self::new(MAX_RUN_DURATION)
    }
}
