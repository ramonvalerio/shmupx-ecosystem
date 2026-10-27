//! Login Google do Lab pelo navegador do sistema (OAuth 2.0 para apps instalados, RFC 8252),
//! o mesmo fluxo do `shmupx-client`: o Rust abre o navegador, recebe o código num listener de
//! loopback, troca o código via backend (o `client_secret` nunca fica no app) e devolve o
//! `id_token` do Google ao React, que entra no Firebase Authentication com ele.
//!
//! Este módulo é uma cópia do `shmupx-client/src-tauri/src/auth`. Se o fluxo mudar, altere os dois.

pub mod commands;
mod oauth;

pub use commands::login_google_desktop;
