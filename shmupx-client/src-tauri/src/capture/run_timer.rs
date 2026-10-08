use std::time::Duration;

/// Duração máxima de uma run (limite superior da janela de 45–60 min, ver spec 002).
pub const MAX_RUN_DURATION: Duration = Duration::from_secs(60 * 60);

/// Duração usada enquanto o timer seguro ainda está em desenvolvimento (testes rápidos).
/// TODO(TASKS.md 2.3): substituir por `max_duration` vindo do servidor ao iniciar a run.
pub const TEST_RUN_DURATION: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunTimerState {
    Idle,
    Running,
    Expired,
    Finished,
}

/// Timer seguro da run, mantido em Rust (não em JS). Corpo pendente (ver TASKS.md 2.1–2.5).
#[derive(Debug)]
pub struct RunTimer {
    pub state: RunTimerState,
    pub limit: Duration,
}

impl RunTimer {
    pub fn new(limit: Duration) -> Self {
        Self {
            state: RunTimerState::Idle,
            limit,
        }
    }
}

impl Default for RunTimer {
    fn default() -> Self {
        Self::new(MAX_RUN_DURATION)
    }
}
