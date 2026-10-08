use std::sync::Arc;

use tokio::process::Child;
use tokio::sync::Mutex;

/// Processo FFmpeg em execução e metadados mínimos para corte/parada.
pub struct RunHandle {
    pub child: Child,
    pub pid: u32,
    pub jogo_id: String,
}

/// Estado global compartilhado (gerenciado pelo Tauri via `tauri::State`).
/// `Arc<Mutex<..>>` porque a task do timer precisa acessá-lo fora do comando que a criou.
#[derive(Clone, Default)]
pub struct RunState(pub Arc<Mutex<Option<RunHandle>>>);
