//! Estruturas de dados tipadas que espelham as tabelas do Supabase (ver `shmupx-backend/supabase/`).

pub mod conhecimento;
pub mod jogo;
pub mod perfil_jogador;

#[allow(unused_imports)] // reexport para quando routes/services existirem (ver TASKS.md)
pub use conhecimento::RobotxConhecimento;
pub use jogo::Jogo;
#[allow(unused_imports)]
pub use perfil_jogador::PerfilJogador;
