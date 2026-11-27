//! Regras de negócio chamadas por `routes/`. `ranking` é puro (sem I/O); `gameplays` lê e grava
//! no Firestore dentro de transações.

pub mod catalogo;
pub mod cliques_download;
pub mod denuncias;
pub mod destaques;
pub mod distribuicoes;
pub mod email;
pub mod estatisticas;
pub mod eventos;
pub mod gameplays;
pub mod jogo_aberto;
pub mod mister;
pub mod notificacoes;
pub mod perfil;
pub mod planos;
pub mod placar;
pub mod ranking;
pub mod recordes;
pub mod robotx;
pub mod tempo_conectado;
pub mod titulos;
pub mod usuarios;
pub mod versoes;
pub mod youtube;
