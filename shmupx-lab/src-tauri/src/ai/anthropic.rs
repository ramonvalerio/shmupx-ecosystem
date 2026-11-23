//! Cliente mínimo da Messages API da Anthropic (HTTP direto: não há SDK oficial para Rust).
//!
//! A chave vem de `ANTHROPIC_API_KEY` no ambiente do PC do admin e nunca é lida de arquivo,
//! enviada à tela, ao backend ou ao Firestore. Só o Rust do Lab fala com a API.

use std::time::Duration;

use serde_json::Value;

const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2026-10-21";
/// Pesquisa na web e pensamento adaptativo podem levar um bom tempo; ainda assim é finito.
const TIMEOUT: Duration = Duration::from_secs(120);

pub const VARIAVEL_CHAVE: &str = "ANTHROPIC_API_KEY";
pub const VARIAVEL_MODELO: &str = "SHMUPX_LAB_AI_MODEL";
/// Suficiente para identificar jogos e bem mais barato que o Opus; troque por
/// `SHMUPX_LAB_AI_MODEL` (ex.: `claude-opus-5-5`) para títulos difíceis.
const MODELO_PADRAO: &str = "claude-sonnet-5-5";
// Em espera: o padrão é a assinatura (`motor.rs`); este cliente só entra com SHMUPX_LAB_AI_PROVIDER=api.

pub struct Config {
    api_key: String,
    pub modelo: String,
}

impl Config {
    pub fn do_ambiente() -> Result<Self, String> {
        let api_key = std::env::var(VARIAVEL_CHAVE)
            .ok()
            .map(|chave| chave.trim().to_string())
            .filter(|chave| !chave.is_empty())
            .ok_or_else(|| {
                format!(
                    "Defina a variável de ambiente {VARIAVEL_CHAVE} e reabra o Lab para usar o preenchimento por IA."
                )
            })?;
        let modelo = std::env::var(VARIAVEL_MODELO)
            .ok()
            .map(|modelo| modelo.trim().to_string())
            .filter(|modelo| !modelo.is_empty())
            .unwrap_or_else(|| MODELO_PADRAO.to_string());
        Ok(Self { api_key, modelo })
    }
}

/// Envia o corpo à Messages API e devolve o JSON da resposta. Os erros HTTP viram mensagens em
/// português para a tela; a chave nunca entra nelas.
pub async fn enviar(config: &Config, corpo: &Value) -> Result<Value, String> {
    let resposta = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("falha ao construir cliente HTTP: {e}"))?
        .post(ENDPOINT)
        .header("x-api-key", &config.api_key)
        .header("anthropic-version", API_VERSION)
        .json(corpo)
        .send()
        .await
        .map_err(|e| format!("falha de rede ao falar com a API da Anthropic: {e}"))?;

    let status = resposta.status();
    if status.is_success() {
        return resposta
            .json()
            .await
            .map_err(|e| format!("resposta da API em formato inesperado: {e}"));
    }
    let detalhe = resposta
        .json::<Value>()
        .await
        .ok()
        .and_then(|corpo| corpo["error"]["message"].as_str().map(str::to_string));
    Err(erro_http(status.as_u16(), detalhe.as_deref()))
}

fn erro_http(status: u16, detalhe: Option<&str>) -> String {
    match status {
        401 | 403 => format!("A API recusou a chave em {VARIAVEL_CHAVE} ({status})."),
        429 => "Limite de uso da API atingido; tente de novo em instantes.".to_string(),
        500..=599 => format!("A API da Anthropic está indisponível no momento ({status})."),
        _ => format!("A API recusou o pedido ({status}): {}", detalhe.unwrap_or("sem detalhe")),
    }
}

/// Texto da resposta: concatena os blocos `text` (ignora busca, raciocínio e demais blocos).
pub fn texto_da_resposta(resposta: &Value) -> String {
    resposta["content"]
        .as_array()
        .map(|blocos| {
            blocos
                .iter()
                .filter(|bloco| bloco["type"] == "text")
                .filter_map(|bloco| bloco["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

pub fn motivo_de_parada(resposta: &Value) -> &str {
    resposta["stop_reason"].as_str().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn junta_so_os_blocos_de_texto() {
        let resposta = json!({ "content": [
            { "type": "thinking", "thinking": "" },
            { "type": "text", "text": "um" },
            { "type": "server_tool_use", "name": "web_search" },
            { "type": "text", "text": "dois" },
        ]});
        assert_eq!(texto_da_resposta(&resposta), "um\ndois");
        assert_eq!(texto_da_resposta(&json!({})), "");
    }

    #[test]
    fn erros_http_tem_mensagem_sem_vazar_chave() {
        assert!(erro_http(401, None).contains(VARIAVEL_CHAVE));
        assert!(erro_http(429, None).contains("Limite"));
        assert!(erro_http(503, None).contains("indisponível"));
        assert!(erro_http(400, Some("campo inválido")).contains("campo inválido"));
    }

    #[test]
    fn motivo_de_parada_ausente_e_vazio() {
        assert_eq!(motivo_de_parada(&json!({ "stop_reason": "end_turn" })), "end_turn");
        assert_eq!(motivo_de_parada(&json!({})), "");
    }
}
