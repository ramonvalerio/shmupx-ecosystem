//! Regras de negócio chamadas por `routes/`. `ranking` é puro (sem I/O); `gameplays` lê e grava
//! no Firestore dentro de transações.

pub mod catalogo;
pub mod email;
pub mod gameplays;
pub mod mister;
pub mod notificacoes;
pub mod ranking;
pub mod versoes;
