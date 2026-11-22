//! MiSTer FPGA pela rede local, usado pelo ShmupX Client (PC) e pelo ShmupX Lite (Android):
//!
//! - [`rede`] (FTP): ler a biblioteca (`.mra` e ROMs), os jogadores do `shmupx.ini` e montar o
//!   `shmupx.ini` com o jogador, o endereço do ShmupX e o token.
//! - [`ssh`]: instalar/atualizar o ShmupX no MiSTer, listar os jogos e modos do menu de um jogador,
//!   abrir um jogo e voltar ao menu.

pub mod descoberta;
pub mod rede;
pub mod ssh;

/// Acesso de fábrica do MiSTer (SSH e FTP): usuário `root`, senha `1`.
pub const USUARIO_PADRAO: &str = "root";
pub const SENHA_PADRAO: &str = "1";

/// Usuário informado ou o padrão (`root`).
pub fn usuario_ou_padrao(usuario: Option<&str>) -> &str {
    usuario.map(str::trim).filter(|u| !u.is_empty()).unwrap_or(USUARIO_PADRAO)
}

/// Senha informada ou a padrão (`1`).
pub fn senha_ou_padrao(senha: Option<&str>) -> &str {
    senha.filter(|s| !s.is_empty()).unwrap_or(SENHA_PADRAO)
}
