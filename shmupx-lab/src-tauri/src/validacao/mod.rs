//! Validação dos scripts por jogo (ADR 004 e 006): o admin reúne gameplays (do YouTube, pelo
//! yt-dlp, ou arquivos locais), informa o placar real de cada uma e roda o script Python do jogo
//! nelas. O Lab mostra o percentual de acerto contra o gabarito e o de aceitação automática
//! (o que o Client aceitaria sem revisão).

pub mod commands;
mod dados;
mod script;
mod ytdlp;
