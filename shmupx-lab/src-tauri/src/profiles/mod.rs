//! Bounded context `profiles`: normalização e exportação local dos perfis de análise
//! mapeados no canvas do Lab (ver docs/specs/001-dominio-analise-e-perfis.md).

pub mod commands;
pub mod types;

pub use commands::exportar_perfil_jogo;
pub use types::{AnalysisProfile, MappingBoxInput, NormalizedRegion};
