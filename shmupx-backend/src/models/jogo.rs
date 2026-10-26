use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Espelha a tabela `public.jogos` (ver `supabase/migrations/20261008120000_init_core_schema.sql`).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Jogo {
    pub id: Uuid,
    pub steam_app_id: i32,
    pub nome: String,
    pub criado_em: DateTime<Utc>,
}
