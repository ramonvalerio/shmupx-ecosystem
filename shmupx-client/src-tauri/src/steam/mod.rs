//! Bounded context `steam`: descobre qual jogo Steam está rodando.
//! Ver TASKS.md (seção Steam) para a implementação pendente.

pub mod detector;

pub use detector::{DetectedGame, GameDetector};
