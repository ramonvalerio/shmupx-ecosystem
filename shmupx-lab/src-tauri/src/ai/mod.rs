//! Preenchimento do catálogo por IA: dado o título de um jogo, propõe desenvolvedora, ano,
//! plataforma, modos e `steam_app_id` para o admin revisar. O Rust do Lab fala direto com a API
//! da Anthropic usando a chave do ambiente do PC (`ANTHROPIC_API_KEY`); nada disso passa pelo
//! backend, pelo Firestore ou pela tela. O cache local evita pagar duas vezes pela mesma consulta.

mod anthropic;
mod cache;
pub mod commands;
mod steam;
mod sugestao;
