//! Regras de negócio chamadas por `routes/`. `ranking` é puro (sem I/O); `gameplays` lê e grava
//! no Firestore dentro de transações.

pub mod catalogo;
pub mod denuncias;
pub mod distribuicoes;
pub mod email;
pub mod gameplays;
pub mod mister;
pub mod notificacoes;
pub mod perfil;
pub mod placar;
pub mod ranking;
pub mod robotx;
pub mod usuarios;
pub mod versoes;
pub mod youtube;
