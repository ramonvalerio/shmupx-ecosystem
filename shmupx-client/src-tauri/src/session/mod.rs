//! Bounded context `session`: credencial (ID token do Firebase Auth) e sala ativa do jogador nesta
//! sessão do app. Guardado em Rust (não só no React) porque fluxos autônomos — o timer seguro
//! expirando, o push-to-talk disparado pelo gamepad — precisam chamar o backend/a Edge Function
//! sem depender de um `invoke` vindo do frontend no momento exato em que isso acontece.

pub mod commands;

use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default)]
pub struct Session {
    pub token: Option<String>,
    pub sala_id: Option<String>,
}

/// `Mutex` padrão (não `tokio::sync::Mutex`): só protege leituras/escritas síncronas rápidas
/// de dois `Option<String>`, nunca mantido através de um `.await`.
#[derive(Clone, Default)]
pub struct SessionState(pub Arc<Mutex<Session>>);

impl SessionState {
    pub fn snapshot(&self) -> Session {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

pub use commands::{definir_sala_ativa, definir_token_autenticacao, limpar_token_autenticacao};
