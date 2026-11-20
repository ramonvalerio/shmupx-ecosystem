//! Bounded context `auth`: login Google pelo navegador do sistema (OAuth 2.0 para apps
//! instalados, RFC 8252). O Rust abre o navegador, recebe o código num listener de loopback
//! efêmero, troca o código usando PKCE e devolve o `id_token` do Google ao React, que entra no
//! Firebase Authentication com ele. Nada de formulário de login dentro da WebView.
//!
//! O token Firebase resultante NÃO é entregue a `session::SessionState`: o `shmupx-backend`
//! atual valida JWT do Supabase e recusaria (ou, pior, interpretaria mal) esse token.

pub mod commands;
pub mod oauth;

pub use commands::login_google_desktop;
