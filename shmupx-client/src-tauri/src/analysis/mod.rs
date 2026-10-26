//! Análise local por scripts de jogo (ADR 004).
//! O pipeline ONNX experimental permanece isolado para referência; não é usado na submissão.

pub mod crop;
pub mod frames;
pub mod model;
pub mod pipeline;
pub mod profile;
pub mod offline;

pub use offline::{analisar_run, AnalysisResult};
