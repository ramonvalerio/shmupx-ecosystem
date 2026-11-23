//! Preenchimento do catálogo por IA: dado o título de um jogo, propõe desenvolvedora, ano,
//! plataforma, modos e `steam_app_id` para o admin revisar. Por padrão usa a assinatura do Claude
//! do admin, pelo Claude Code instalado no PC (`assinatura.rs`); a Messages API com chave
//! (`anthropic.rs`) fica em espera (`motor.rs`). Nada disso passa pelo backend, pelo Firestore ou
//! pela tela. O cache local evita repetir a mesma consulta.

pub(crate) mod anthropic;
mod assinatura;
mod motor;
mod cache;
pub mod commands;
mod steam;
mod sugestao;
