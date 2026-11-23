//! Por onde o Lab fala com o Claude. Padrão: a assinatura do admin, pelo Claude Code instalado no
//! PC (`assinatura.rs`). A Messages API com chave (`anthropic.rs`) fica em espera: só entra com
//! `SHMUPX_LAB_AI_PROVIDER=api` (e `ANTHROPIC_API_KEY`).

use super::anthropic::{Config, VARIAVEL_MODELO};

pub const VARIAVEL_PROVEDOR: &str = "SHMUPX_LAB_AI_PROVIDER";
/// O mesmo modelo dos dois caminhos; troque por `SHMUPX_LAB_AI_MODEL` (ex.: `claude-opus-5-5`).
const MODELO_PADRAO: &str = "claude-sonnet-5-5";

pub enum Motor {
    /// Claude Code logado na assinatura claude.ai do admin.
    Assinatura { modelo: String },
    /// Messages API com `ANTHROPIC_API_KEY` (em espera).
    Api(Config),
}

impl Motor {
    pub fn do_ambiente() -> Result<Self, String> {
        let provedor = std::env::var(VARIAVEL_PROVEDOR).unwrap_or_default();
        if provedor.trim().eq_ignore_ascii_case("api") {
            return Ok(Self::Api(Config::do_ambiente()?));
        }
        let modelo = std::env::var(VARIAVEL_MODELO)
            .ok()
            .map(|m| m.trim().to_string())
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| MODELO_PADRAO.to_string());
        Ok(Self::Assinatura { modelo })
    }

    pub fn modelo(&self) -> &str {
        match self {
            Self::Assinatura { modelo } => modelo,
            Self::Api(config) => &config.modelo,
        }
    }
}
