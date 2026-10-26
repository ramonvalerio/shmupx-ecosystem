# shmupx-frontend

Portal web público (Next.js App Router + Tailwind 4) — ranking global e catálogo de jogos.

- Nota importante da própria versão do Next (16, Cache Components habilitado): ver `AGENTS.md`
  antes de alterar `app/`. Fetches dinâmicos precisam ficar atrás de `<Suspense>`.
- `src/app/`: `/` (leaderboard global, mock), `/jogos` (catálogo real via `GET /api/jogos`),
  `/jogos/[jogoId]` (placar mock de um jogo real).
- `src/services/api.ts`: cliente HTTP para o `shmupx-backend` (Axum, porta 3000 por padrão;
  configurável via `NEXT_PUBLIC_API_BASE_URL`).
- Ver `TASKS.md` para o que é mock vs. real hoje, e o roteiro de telas futuras.

```bash
pnpm install
pnpm dev     # http://localhost:3000 (precisa do shmupx-backend rodando para dados reais em /jogos)
pnpm build && pnpm start
```
