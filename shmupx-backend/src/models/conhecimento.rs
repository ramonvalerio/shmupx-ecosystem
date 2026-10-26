use serde::{Deserialize, Serialize};

/// Espelha a tabela `public.robotx_conhecimento`. Deliberadamente **sem** o campo `embedding`:
/// ele só é lido pela RPC `buscar_conhecimento_robotx` dentro do Postgres, nunca por esta API
/// diretamente — é essa fronteira que garante o isolamento de RAG por jogo (ver ADR 003).
#[allow(dead_code)] // ainda sem rota/service consumindo este model (ver TASKS.md)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RobotxConhecimento {
    pub id: String,
    pub jogo_id: String,
    pub conteudo: String,
    pub criado_em: String,
}
