# shmupx-backend

API nativa em Rust (Axum + sqlx) conectada ao Postgres do Supabase.

- `supabase/migrations/`: schema (jogos, perfis de análise, base vetorial do RobotX, economia de
  fichas, presença de salas) — ver `docs/adr/003` e `docs/specs/001`/`002`.
- `src/`: servidor Axum (`routes/`, `models/`, `services/`). Hoje expõe `GET /health` e
  `GET /api/jogos`; demais rotas (ranking, runs, publicação de perfil) ainda não implementadas —
  ver `TASKS.md`.
- `.sqlx/`: cache offline do `sqlx` (gerado via `cargo sqlx prepare`) — permite `cargo check`/
  `build` sem um Postgres vivo. Regenerar sempre que uma query mudar ou uma nova for adicionada.

```bash
export DATABASE_URL=postgres://usuario:senha@host:5432/postgres   # Supabase ou Postgres local
cargo run   # sobe em http://localhost:3000
```

Sem `DATABASE_URL`, o processo imprime as instruções acima no terminal e sai (não entra num loop
de panic). `cargo check`/`cargo build` funcionam sem a variável definida, usando o cache `.sqlx/`.
