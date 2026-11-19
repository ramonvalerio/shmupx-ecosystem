//! Análise local por scripts de jogo (ADR 004).
//! O pipeline ONNX experimental permanece isolado para referência; não é usado na submissão.

pub mod crop;
pub mod frames;
pub mod model;
pub mod offline;
pub mod pipeline;
pub mod profile;

pub use offline::{AnalysisResult, analisar_run};
