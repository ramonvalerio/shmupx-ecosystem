use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Bounding box em pixels absolutos, como editado no canvas do frontend (`types/mapping.ts`).
#[derive(Debug, Clone, Deserialize)]
pub struct MappingBoxInput {
    pub key: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Região normalizada (0..1), formato consumido pelo motor ONNX no Client (ver docs/adr/002).
#[derive(Debug, Clone, Serialize)]
pub struct NormalizedRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Perfil de análise versionado (Spec 001). `model_ref`/`sha256` ficam de fora até o passo
/// de publicação (Supabase Storage + `profile_releases`) — fora de escopo desta exportação local.
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisProfile {
    pub game_id: String,
    pub version: String,
    pub variant: String,
    pub regions: BTreeMap<String, NormalizedRegion>,
    /// Unix timestamp (segundos) de quando o perfil foi exportado.
    pub created_at: u64,
}
