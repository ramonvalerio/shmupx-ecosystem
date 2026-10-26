//! Camada de rede: clientes HTTP para o `shmupx-backend` (Axum) e a Edge Function `robotx-voice`
//! do Supabase, e reprodução do áudio de resposta.

pub mod backend;
pub mod playback;
pub mod robotx;
