use serde::{Deserialize, Serialize};

/// Espelha a tabela `public.perfis_jogadores`: saldo de fichas digitais e índice global
/// de ranking (ver docs/specs/002-economia-fichas-e-ranking.md).
#[allow(dead_code)] // ainda sem rota/service consumindo este model (ver TASKS.md)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfilJogador {
    pub id: String,
    pub fichas_disponiveis: i32,
    pub pontuacao_global_normalizada: f64,
    pub criado_em: String,
    pub atualizado_em: String,
}
