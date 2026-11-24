//! Bounded context `steam`: descobre qual jogo Steam está rodando.
//! Ver TASKS.md (seção Steam) para a implementação pendente.

pub mod biblioteca;
pub mod commands;
pub mod controles;
pub mod detector;
pub mod emulador;
pub mod mister;
pub mod mister_ssh;
pub mod roms_auto;
pub mod telemetria;
pub mod treino;
pub mod trailer;

pub use commands::inicializar_jogo_steam;
pub use detector::{DetectedGame, GameDetector, is_steam_running};
