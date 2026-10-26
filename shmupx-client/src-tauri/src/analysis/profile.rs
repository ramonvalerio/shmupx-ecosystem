use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::{AppHandle, Manager};

/// Região normalizada (0..1) — espelha `Region` em `shmupx-lab/src/types/mapping.ts` e
/// `NormalizedRegion` em `shmupx-lab/src-tauri/src/profiles/types.rs`.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Region {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Espelha o JSON exportado por `shmupx-lab` (`profiles::commands::exportar_perfil_jogo`):
/// `{ game_id, version, variant, regions: { score, lives?, continues?, gameOver? }, created_at }`.
#[derive(Debug, Clone, Deserialize)]
pub struct AnalysisProfile {
    pub game_id: String,
    pub version: String,
    pub variant: String,
    pub regions: HashMap<String, Region>,
    pub created_at: u64,
}

/// Caminho local convencional de um perfil sincronizado. **Não existe sincronização real
/// ainda** (Spec 001, passo 6 — "Sincronizar (Client)"; ver TASKS.md) — isto só funciona hoje
/// se alguém copiar manualmente um `profile.json` exportado pelo Lab para este caminho.
pub fn caminho_local(app: &AppHandle, jogo_id: &str) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("não foi possível resolver a pasta de dados do app: {e}"))?;

    Ok(data_dir.join("profiles").join(jogo_id).join("profile.json"))
}

/// Carrega e desserializa o perfil de `path`. Qualquer falha (arquivo ausente, JSON
/// corrompido/formato inesperado) vira `Err` com uma mensagem que já explica a causa mais
/// provável, em vez de um erro críptico de serde.
pub fn carregar(path: &Path) -> Result<AnalysisProfile, String> {
    let conteudo = std::fs::read_to_string(path).map_err(|e| {
        format!(
            "perfil de análise não encontrado em {} ({e}) — a sincronização automática de \
             perfis ainda não existe; copie manualmente um profile.json exportado pelo Lab \
             para esse caminho para testar",
            path.display()
        )
    })?;

    serde_json::from_str(&conteudo).map_err(|e| {
        format!(
            "perfil de análise em {} está corrompido ou num formato inesperado: {e}",
            path.display()
        )
    })
}
