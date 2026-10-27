use std::path::PathBuf;
use std::sync::Arc;

use tokio::process::Child;
use tokio::sync::Mutex;

/// Processo FFmpeg em execução e metadados mínimos para corte/parada/análise/submissão ao
/// backend. `run_id`/`token` são uma cópia do que o backend devolveu/do que a sessão tinha no
/// momento de `iniciar_corrida_safe` — guardados aqui (em vez de reconsultados depois) para o
/// timer de expiração e `finalizar_corrida_manual` não dependerem da sessão não ter mudado
/// nesse meio-tempo. `video_path` é o `.mp4` que o pipeline de análise (`analysis::analisar_run`)
/// vai processar depois que o FFmpeg fechar o arquivo.
pub struct RunHandle {
    pub child: Child,
    pub pid: u32,
    pub jogo_id: String,
    pub run_id: String,
    pub token: String,
    pub video_path: PathBuf,
}

/// Fila das análises: uma por vez (são pesadas), em ordem de chegada (o `Mutex` do Tokio é
/// justo). Fica separada de `RunState`: uma nova run pode começar enquanto outra é analisada.
#[derive(Clone, Default)]
pub struct AnalysisQueue(pub Arc<Mutex<()>>);

/// Estado global compartilhado (gerenciado pelo Tauri via `tauri::State`).
/// `Arc<Mutex<..>>` porque a task do timer precisa acessá-lo fora do comando que a criou.
#[derive(Clone, Default)]
pub struct RunState(pub Arc<Mutex<Option<RunHandle>>>);
