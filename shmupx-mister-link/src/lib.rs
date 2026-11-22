//! MiSTer FPGA pela rede local, usado pelo ShmupX Client (PC) e pelo ShmupX Lite (Android):
//!
//! - [`rede`] (FTP): ler a biblioteca (`.mra` e ROMs), os jogadores do `shmupx.ini` e montar o
//!   `shmupx.ini` com o jogador, o endereço do ShmupX e o token.
//! - [`ssh`]: instalar/atualizar o ShmupX no MiSTer, listar os jogos e modos do menu de um jogador,
//!   abrir um jogo e voltar ao menu.

pub mod rede;
pub mod ssh;
