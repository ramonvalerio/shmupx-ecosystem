# ShmupX Frontend — Tarefas de implementação (Portal Web Público)

Contexto: `docs/adr/001` (ADR já define Next.js como stack deste app), `docs/specs/002`.

## 0. Inicialização do projeto
- [x] 0.1 `pnpm create next-app@latest .` (App Router, TypeScript, Tailwind 4, ESLint,
      `src/`, import alias `@/*`, `--use-pnpm`) — usei `pnpm` em vez de `npx`/`npm` de propósito:
      nas tarefas anteriores do client/lab, rodar `npx` nesta máquina acionava o corepack e
      sujava `package.json` com um campo `packageManager` indesejado; `pnpm` direto evita isso.
- [x] 0.2 **Breaking changes da v16 lidas antes de codar**: o próprio scaffold cria um
      `AGENTS.md` avisando que esta versão do Next tem mudanças de API em relação a
      conhecimento prévio, e aponta para `node_modules/next/dist/docs/`. O scaffold já vem com
      `cacheComponents: true` + `partialPrefetching: true` no `next.config.ts` (padrão
      recomendado do `create-next-app`) — isso exige que todo fetch dinâmico/sem cache fique
      atrás de um `<Suspense>` (não dá para só `await fetch()` direto no topo de uma page como
      em versões anteriores). Toda a estrutura abaixo já foi desenhada em torno disso, não como
      ajuste posterior.

## 1. Identidade visual
- [x] 1.1 Tema escuro (`zinc-950`), fonte pixelada **Press Start 2P** via `next/font/google`
      (self-hosted, sem request externo) aplicada só em títulos/scores/badges — não no texto
      corrido (legibilidade). Acentos neon: âmbar (1º lugar/CTA principal), fúcsia e ciano
      (navegação/estados secundários) — paleta propositalmente distinta das já usadas em
      `shmupx-client` (verde/roxo) e `shmupx-lab` (ciano), para o portal público ter identidade
      própria dentro do mesmo sistema visual "arcade".
- [x] 1.2 `PodiumTop3` (pódio visual com medalhas 🥇🥈🥉) + `GlobalLeaderboardTable` com
      destaque de borda/sombra nas 3 primeiras posições — a referência "TOP 3 LEADERBOARD" de
      fliperama pedida na tarefa.

## 2. Leaderboard Global (`/`)
- [x] 2.1 Tabela ordenada por `pontuacaoGlobalNormalizada` (0.00–10.00): posição, avatar
      estilizado (iniciais + cor determinística por hash, sem depender de upload/URL de
      imagem), nome, média global, top 3 jogos.
- [ ] 2.2 **Mockado por completo, de propósito** (`data/mockRanking.ts`) — o backend já tem
      `GET /api/rankings/global` (soma dos 5 jogos mais jogados por jogador, ver
      `shmupx-backend/TASKS.md` 3.11), mas ele devolve só `usuario_id` (UUID) + a soma; ainda
      falta, para esta tela virar real:
      - uma coluna `display_name` (ou join com `auth.users`/uma tabela de perfil público) em
        `perfis_jogadores`, hoje inexistente — sem isso não dá para mostrar nome/avatar;
      - o "top 3 jogos que o jogador domina" não é algo que `GET /api/rankings/global` calcula
        (ele soma os 5 mais jogados, não expõe quais são) — precisaria devolver essa lista
        também, ou um endpoint complementar.

## 3. Catálogo de jogos (`/jogos`) e placar por jogo (`/jogos/[jogoId]`)
- [x] 3.1 `/jogos` consulta **de verdade** `GET /api/jogos` do `shmupx-backend`
      (`services/api.ts::getJogos`, `fetch` com `cache: "no-store"` — o catálogo pode mudar a
      qualquer momento). Fica atrás de `<Suspense>` (`JogosListSkeleton` como fallback), exigido
      pelo modo Cache Components.
- [x] 3.2 Se o backend estiver fora do ar, cai para `data/mockJogos.ts` com um aviso visível na
      tela ("exibindo dados de exemplo") em vez de quebrar a página — mas a integração real **é**
      a real: validado de ponta a ponta subindo um Postgres + o binário do `shmupx-backend` de
      verdade e confirmando que os 3 jogos reais aparecem (nomes reais, sem o aviso de mock).
- [x] 3.3 `/jogos/[jogoId]`: resolve o jogo real pelo `id` (reaproveitando `getJogos()`), chama
      `notFound()` se o id não existir. Placar bruto (`ScoreTable`) é **mockado** e
      determinístico por `jogo.id` (`data/mockScores.ts::gerarPlacarMock`) — não existe
      `GET /api/jogos/:id/scores` no backend ainda.
- [ ] 3.4 **Limitação conhecida de `notFound()` sob Partial Prerendering**: testado de verdade
      com um `jogoId` inexistente — o conteúdo renderizado está correto (mostra a página de
      "não encontrado"), mas o **status HTTP volta 200**, não 404. Isso acontece porque a rota é
      servida como shell estático (200 enviado de imediato) com o conteúdo dinâmico
      streamado depois via `<Suspense>` — quando o `notFound()` dispara, o cabeçalho HTTP já foi
      enviado e não pode mais ser trocado. É uma característica do modelo de streaming/PPR do
      Next 16, não um bug de lógica desta página; crawlers que dependem do status code (não do
      conteúdo) podem ser afetados. Mitigação futura: servir esta rota fora do Cache Components
      (`export const dynamic = "force-dynamic"` só nesta rota) se o status HTTP precisar ser
      exato — não aplicada agora para não abrir uma exceção ao padrão do resto do app sem
      necessidade concreta ainda.

## 4. Conectividade com o backend
- [x] 4.1 `services/api.ts`: `API_BASE_URL` configurável via `NEXT_PUBLIC_API_BASE_URL` (default
      `http://localhost:3000`), `getJogos()` tipado contra `types/jogo.ts` (espelha o JSON real
      do `sqlx`/`serde` do Rust), `ApiError` para erros de HTTP.
- [ ] 4.2 Sem camada de cliente HTTP compartilhada para POST (`/api/runs/start`, `/api/runs`) —
      só a leitura (`GET /api/jogos`) foi pedida nesta tarefa; a submissão de runs é feita pelo
      `shmupx-client` (Tauri), não por este portal web.

## 5. Validação feita nesta tarefa
- `pnpm exec tsc --noEmit` limpo (precisou rodar `pnpm exec next typegen` primeiro — o helper
  `PageProps<'/rota'>` só existe para rotas já conhecidas pelo gerador de tipos do Next).
- `pnpm build` limpo: `/` estático, `/jogos` e `/jogos/[jogoId]` como Partial Prerender (shell
  estático + conteúdo dinâmico via `Suspense`) — exatamente o resultado esperado sob Cache
  Components para páginas com fetch dinâmico.
- `pnpm lint` limpo.
- **Integração real ponta a ponta**, não só mock: subi um Postgres 16 + pgvector via Docker,
  apliquei as 3 migrations do `shmupx-backend`, semeei 3 jogos reais, rodei o binário do
  `shmupx-backend` de verdade (`cargo build` + execução) e o `shmupx-frontend` em modo produção
  (`pnpm build && pnpm start`, porta separada para não colidir com o backend) — `/jogos` mostrou
  os 3 jogos reais do Postgres, sem o aviso de mock; `/jogos/[idReal]` mostrou o nome real do
  jogo com o placar mock por baixo; `/jogos/[idInexistente]` mostrou o conteúdo de 404 correto
  (com a limitação de status HTTP documentada em 3.4).

## 6. Roteiro de próximas telas (pedido explicitamente nesta tarefa)
- [ ] 6.1 **Perfil do Jogador** (`/jogadores/[id]`): histórico pessoal, fichas restantes,
      índice global, jogos favoritos — depende de 2.2 (o backend precisar expor nome/avatar) e
      de autenticação (6.3) para a versão "meu perfil" vs. perfil público de outro jogador.
- [ ] 6.2 **Histórico de Runs da Temporada** (`/jogadores/[id]/runs` ou aba dentro do perfil):
      lista as runs de `public.runs` (status `valida`/`expirada`/`invalida`, score, nota,
      continues) — a tabela já existe no backend (`shmupx-backend/supabase/migrations/
      20261008140000_add_runs.sql`); falta só uma rota `GET /api/runs?usuario_id=...` para
      consumir.
- [ ] 6.3 **Integração com Supabase Auth**: login do jogador no portal (hoje 100% público/sem
      sessão) para ver o próprio perfil/histórico e, futuramente, interagir com o site logado.
      Precisa decidir se o portal fala com o Supabase Auth direto (client-side) ou se o
      `shmupx-backend` intermedia — ainda não decidido, está fora do escopo desta tarefa.
- [ ] 6.4 Filtro/busca na lista de jogos (`/jogos`) quando o catálogo crescer além de poucos
      itens — hoje é uma grade simples sem paginação/busca.
