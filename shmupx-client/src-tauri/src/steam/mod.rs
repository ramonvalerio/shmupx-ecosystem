//! Bounded context `steam`: descobre qual jogo Steam está rodando.
//! Ver TASKS.md (seção Steam) para a implementação pendente.

pub mod commands;
pub mod detector;

pub use commands::inicializar_jogo_steam;
pub use detector::{is_steam_running, DetectedGame, GameDetector};
