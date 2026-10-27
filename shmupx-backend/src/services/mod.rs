//! Regras de negócio chamadas por `routes/`. `ranking` é puro (sem I/O); `gameplays` lê e grava
//! no Firestore dentro de transações.

pub mod gameplays;
pub mod ranking;
