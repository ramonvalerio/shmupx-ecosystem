# ShmupX Backend — Tarefas de implementação (Supabase)

Contexto: `docs/adr/001`, `docs/adr/002`, `docs/adr/003`, `docs/specs/001`, `docs/specs/002`.

## 0. Projeto Supabase
- [ ] 0.1 Criar o projeto Supabase (ou `supabase init` local) e rodar `supabase link --project-ref <ref>`.
- [x] 0.2 Estrutura de migrations em `supabase/migrations/` (convenção padrão do Supabase CLI).
- [ ] 0.3 Configurar `OPENAI_API_KEY` nos secrets do projeto (`supabase secrets set OPENAI_API_KEY=...`)
      — obrigatório para `robotx-voice` rodar (Whisper, embeddings, chat, TTS). `SUPABASE_URL` e
      `SUPABASE_SERVICE_ROLE_KEY` são injetadas automaticamente pela plataforma, não precisam de
      configuração manual. Deploy: `supabase functions deploy robotx-voice`.
- [ ] 0.4 Configurar `SUPABASE_JWT_SECRET` no ambiente do `shmupx-backend` (Axum) — necessário
      para `POST /api/runs/start` e `POST /api/runs` validarem o token do jogador (ver 3.9).
      No painel do Supabase: Project Settings → API → JWT Settings → "JWT Secret" (esquema
      legado HS256; não confundir com `SUPABASE_SERVICE_ROLE_KEY`, que é outra chave).

## 1. Schema inicial (`20261008120000_init_core_schema.sql`)
- [x] 1.1 Extensões `vector` e `uuid-ossp` no schema `extensions` (idempotente via `CREATE EXTENSION IF NOT EXISTS`).
- [x] 1.2 Tabelas `jogos` e `perfil_versoes` (releases imutáveis por `jogo_id + versao + variante`, Spec 001).
- [x] 1.3 Tabela `robotx_conhecimento` (chunks + `embedding VECTOR(1536)`, FK `ON DELETE CASCADE` para `jogos`).
- [x] 1.4 RPC `buscar_conhecimento_robotx(param_jogo_id, param_embedding, param_limite)`: `SECURITY DEFINER`, filtro rígido `WHERE jogo_id = param_jogo_id`, similaridade de cosseno via `1 - (embedding <=> param_embedding)`. `EXECUTE` revogado de `public`, concedido só a `service_role`.
- [x] 1.5 Tabelas `perfis_jogadores` (fichas + pontuação, Spec 002) e `salas_online` (presença por jogo/jogador).
- [x] 1.6 Trigger `handle_new_user` em `auth.users`: cria `perfis_jogadores` com 5 fichas automaticamente no cadastro.
- [x] 1.7 Índices nas FKs (`perfil_versoes.jogo_id`, `robotx_conhecimento.jogo_id`, `salas_online.jogo_id/jogador_id`); índice HNSW do `embedding` deixado **comentado** no SQL — criar manualmente (`CREATE INDEX CONCURRENTLY`) quando o volume de `robotx_conhecimento` justificar o custo de build.
- [x] 1.8 RLS habilitado em todas as tabelas novas: `jogos`/`perfil_versoes` com leitura pública; `robotx_conhecimento` **sem** policy de leitura para `anon`/`authenticated` (só acessível via a RPC `SECURITY DEFINER` — é isso que impede um client autenticado de ler o conhecimento de outro jogo via PostgREST direto); `perfis_jogadores`/`salas_online` restritos ao próprio `auth.uid()`.

## 1b. Lacuna corrigida: tabela `salas` (`20261008130000_add_salas.sql`)
- [x] 1b.1 A migration inicial só modelava `salas_online` (presença: `jogo_id` + `jogador_id`),
      sem nenhuma entidade de "sala" com ID próprio. Isso travava a implementação da
      Edge Function `robotx-voice`, que recebe `sala_id` e precisa resolver o `jogo_id`
      autoritativo **no servidor** (ADR 003 — nunca confiar no `jogo_id` que o cliente mande).
      Criada `public.salas (id, jogo_id, criado_em)`; `salas_online` ganhou a coluna
      `sala_id` (FK opcional, não quebra dados existentes). RLS com leitura pública, escrita só
      via `service_role`.
- [x] 1b.2 Validado de verdade: migration aplicada em sequência sobre um Postgres 16 + pgvector
      limpo, idempotência confirmada rodando as duas migrations de novo sem erro, e um `INSERT`
      + `JOIN` real provando o lookup `sala_id -> jogo_id`.

## 1c. Lacuna corrigida: tabela `runs` / ficha ativa (`20261008140000_add_runs.sql`)
- [x] 1c.1 Pré-requisito para a antifraude de `POST /api/runs`: sem uma tabela que registre a
      "ficha ativa" aberta em `POST /api/runs/start`, não haveria nada contra o que validar o
      timestamp de encerramento. Criada `public.runs (id, jogador_id, jogo_id,
      server_started_at, max_duration_segundos, status, score_bruto, vidas_perdidas,
      continues_usados, nota_normalizada, submetida_em)`; `jogos` ganhou
      `max_duration_segundos` (default 3600s = 60min, `CHECK` 1–7200s, Spec 002: janela de
      45–60min por jogo). RLS com `SELECT` restrito ao próprio `auth.uid()`; **sem** policy de
      `INSERT`/`UPDATE` para `anon`/`authenticated` — todo write em `runs` passa pela API Axum
      (validação antifraude em Rust), nunca direto do cliente via PostgREST.
- [x] 1c.2 Validado de verdade sobre Postgres 16 + pgvector real: as três migrations aplicadas em
      sequência + idempotência confirmada rodando as três de novo; `nota_normalizada` começou
      como `NUMERIC(4,2)` e foi trocada para `DOUBLE PRECISION` depois que o `cargo check`
      mostrou que precisaria da feature opcional `bigdecimal`/`rust_decimal` do sqlx só para
      bindar um `f64` — mais simples manter o tipo nativo já que a matemática é feita em ponto
      flutuante no Rust mesmo.

### Como aplicar
```bash
# contra o projeto remoto já linkado
supabase db push

# ou localmente, com `supabase start` rodando
supabase migration up
```

### Validação feita nesta tarefa
A migration foi executada (não só revisada) contra um Postgres 16 + pgvector real via Docker
(`pgvector/pgvector:pg16`), com stubs mínimos de `auth.users`, dos papéis `anon`/`authenticated`/
`service_role` e de `auth.uid()` (esses existem de fábrica num projeto Supabase real — não fazem
parte desta migration). Confirmado:
- Roda limpa do zero e **de novo em cima de si mesma** (idempotência real, não só leitura do SQL).
- O trigger de novo usuário cria `perfis_jogadores` com `fichas_disponiveis = 5`.
- `buscar_conhecimento_robotx` retorna **apenas** o conteúdo do `jogo_id` pedido mesmo quando o
  embedding de busca é idêntico ao de um registro de outro jogo — o isolamento da ADR-003 não
  depende da similaridade vetorial, é um filtro WHERE incondicional.

## 2. Decisão de arquitetura: Axum (Rust) em vez de Edge Functions para a maior parte da API
A API deixou de ser uma coleção de Supabase Edge Functions (Deno) e passou a ser um serviço
Axum/Rust próprio em `shmupx-backend/src/`, conectando direto no Postgres do Supabase via `sqlx`.
Isso muda o destino dos itens abaixo (antes listados como Edge Functions):

- [x] 2.1 **Exceção deliberada, implementada:** `robotx-voice` (`supabase/functions/robotx-voice/index.ts`,
      Deno) continua como Edge Function Supabase em vez de rota Axum (mais perto do Storage/Realtime).
      Pipeline completo: recebe `multipart/form-data` (`audio` .wav, `sala_id`, e um `jogo_id` que é
      **lido mas nunca usado para autorização** — resolve o `jogo_id` de verdade via `salas` no
      servidor, ver 1b) → Whisper (`whisper-1`) transcreve → `text-embedding-3-small` gera o
      embedding da pergunta → RPC `buscar_conhecimento_robotx` (service role) busca o contexto
      isolado por jogo → prompt mestre (persona RobotX + restrição a responder só com o contexto) →
      `gpt-4o-mini` → TTS (`tts-1`) → responde com `audio/mpeg` no corpo (200) e o texto da
      transcrição/resposta em headers (`X-Robotx-Transcript`/`X-Robotx-Answer`, URL-encoded).
      Fallbacks em áudio (ou texto, se o próprio TTS falhar) para: sem contexto no RAG, transcrição
      vazia, e qualquer falha de API externa (Whisper/embedding/RPC/chat/TTS) — nunca propaga um
      erro não tratado. Reavaliar se migrar para uma rota Axum quando o resto da API estiver maduro.
      **Validado de verdade:** `deno check`/`deno lint` limpos contra os SDKs reais (via Docker,
      sem Deno instalado localmente); servidor rodado e testado com `curl` — `405` em método
      errado, `200` no preflight `OPTIONS`, `400` em áudio ausente/`sala_id` inválido, e `500`
      limpo (sem stack trace exposto) quando a resolução da sala falha de verdade (testado
      forçando um erro de DNS contra uma URL de Supabase fake). Não testado com credenciais reais
      da OpenAI/Supabase (exigiria chaves de API ativas).
- [ ] 2.2 `profile-publish` (Spec 001) → vira rota Axum (`POST /perfis/publicar` ou similar, ver 3.x):
      recebe `profile.json` + `model.onnx` do Lab, calcula SHA-256, envia ao Supabase Storage e
      grava em `perfil_versoes`.
- [ ] 2.3 `run-start`/`run-submit` (Spec 002) → vira rota Axum (ver 3.x).
- [ ] 2.4 Job periódico de ranking (Spec 002, seção 3) → função pura em `services/ranking.rs`
      chamada por uma rota/cron, não mais uma Edge Function isolada.

## 3. API Axum (`src/`) — scaffolding feito nesta etapa
- [x] 3.1 `cargo init` executado direto em `shmupx-backend/`; dependências em `Cargo.toml`:
      `axum`, `tokio` (feature `full`), `serde`/`serde_json`, `tower-http` (`cors`, `trace`), `sqlx`
      (`postgres`, `runtime-tokio-native-tls`, `json`, `uuid`, `chrono` — as duas últimas features
      entraram no passo 3.6, junto com as crates `uuid`/`chrono`, quando a primeira query real
      precisou mapear colunas `UUID`/`TIMESTAMPTZ`).
- [x] 3.2 Módulos `src/routes/` (controladores), `src/models/` (`Jogo`, `PerfilJogador`,
      `RobotxConhecimento` — espelhando as tabelas da migration; `embedding` deliberadamente fora
      de `RobotxConhecimento`, só acessível via RPC) e `src/services/` (vazio por ora, primeiro
      morador previsto é o algoritmo de ranking).
- [x] 3.3 `src/main.rs`: servidor Axum na porta 3000, `CorsLayer::permissive()` (dev local — trocar
      por allowlist antes de produção) e `TraceLayer::new_for_http()`. `GET /health` testado de
      verdade (processo rodado e `curl` respondendo `{"status":"ok"}` com HTTP 200).
- [x] 3.4 `cargo check`/`cargo build` sem warnings (models ainda não usados foram marcados
      `#[allow(dead_code)]`/`#[allow(unused_imports)]` explicitamente, já que nenhuma rota/service
      os consome nesta etapa — não é código morto por acidente).
- [x] 3.5 `sqlx::PgPool` real em `main.rs`: lê `DATABASE_URL` do ambiente (`env::var`), e se estiver
      ausente imprime instruções claras no terminal e sai com `exit(1)` em vez de panicar com stack
      trace; mesmo tratamento se a conexão falhar (URL configurada mas banco inacessível). Pool
      criado via `PgPoolOptions::new().max_connections(10).connect(...)` e injetado com
      `.with_state(pool)` no router (depois de mesclar `routes::health` + `routes::jogos`, que por
      isso teve que virar `Router<S>` genérico — ver `routes/health.rs`).
- [x] 3.6 `GET /api/jogos` (`routes/jogos.rs`): `sqlx::query_as!(Jogo, "SELECT id, steam_app_id,
      nome, criado_em FROM public.jogos ORDER BY nome")`, checada em **tempo de compilação** contra
      um Postgres real (não só contra o cache — ver nota de validação abaixo). `Jogo` agora usa
      `uuid::Uuid`/`chrono::DateTime<Utc>` nos campos (motivo pelo qual essas dependências entraram
      no `Cargo.toml` só agora, conforme o TODO que eu mesmo deixei no item 3.5 anterior).
- [x] 3.6b Erros de banco em runtime (`src/error.rs::AppError`, `From<sqlx::Error>`) retornam
      `500 {"error": "..."}` em vez de panicar — testado de verdade renomeando a tabela `jogos` em
      runtime e confirmando que `/api/jogos` deu 500 limpo enquanto `/health` continuou respondendo
      no mesmo processo.
- [x] 3.7 `POST /api/runs/start` e `POST /api/runs` (`routes/runs.rs`): débito atômico de ficha
      (transação com `SELECT ... FOR UPDATE` para não haver corrida entre duas chamadas
      concorrentes do mesmo jogador) + criação da "ficha ativa" em `runs`; submissão exige essa
      ficha ativa (`status = 'ativa'`), valida a janela `server_started_at + max_duration_segundos
      + 120s de tolerância` contra `now()` no servidor (nunca confiando em timestamp alegado pelo
      Client), marca `expirada` e devolve `409` se passou da janela, senão calcula a nota e marca
      `valida`. **Fora de escopo desta tarefa** (ainda pendente): validação de hash do vídeo —
      isso depende do Client enviar o hash (TASKS.md do client, 3.7), e status `needs_review`
      (Spec 001: confiança abaixo de um limiar na análise ONNX) — nenhuma análise real chega até
      aqui ainda, só o payload que o task pediu (`score_bruto`/`vidas_perdidas`/`continues_usados`).
- [ ] 3.8 `GET /ranking/:jogo_id` — depende de uma tabela/view de ranking agregado (Spec 002,
      seção 3: Z-score **por temporada**; a implementação atual em `services::ranking` não é
      ainda escopada por temporada, ver 4.4).
- [x] 3.9 Middleware de autenticação JWT (`middleware/auth.rs::exigir_usuario_autenticado`),
      aplicado via `route_layer` só no router de `routes/runs.rs` (`/api/runs/start`,
      `/api/runs`) — `/health`, `/api/jogos` e `/api/rankings/global` continuam públicas,
      sem tocar no roteador principal em `main.rs`. Valida `Authorization: Bearer <token>`
      (HS256, segredo em `SUPABASE_JWT_SECRET`), extrai o claim `sub` como `Uuid` e injeta via
      `req.extensions_mut().insert(usuario_id)`; os handlers passaram a receber
      `Extension<Uuid>` em vez de ler `usuario_id` do corpo JSON — **o campo foi removido dos
      dois payloads** (`IniciarRunPayload`/`SubmeterRunPayload`), então não é mais só "ignorado
      na prática", é impossível de enviar.
      **Limitação documentada, não escondida:** só HS256 (segredo compartilhado — o esquema
      legado do Supabase). Projetos novos do Supabase podem assinar com RS256/chave assimétrica
      via JWKS, o que exigiria buscar/cachear chaves públicas em vez de um segredo estático —
      não implementado.
      **Validado de ponta a ponta, com tentativa de ataque de verdade:** sem header → `401`;
      token assinado com segredo errado → `401`; token válido → `201` com o `jogador_id`
      gravado no banco batendo com o `sub` do token; e um teste explícito enviando
      `usuario_id` de outra pessoa no corpo junto com um token válido — a run ficou gravada no
      jogador do token, confirmando que o campo forjado não tem efeito nenhum (nem silenciosamente
      ignorado por acaso: ele não existe mais no struct). 7 testes unitários puros (sem
      Postgres) cobrindo token válido, segredo errado, expirado, malformado, header
      ausente/sem prefixo `Bearer` — a validação de assinatura/claims foi isolada numa função
      que recebe o segredo como parâmetro em vez de ler `env::var` direto, exatamente para os
      testes não precisarem mexer em variável de ambiente global compartilhada entre testes em
      paralelo.
- [x] 3.10 `services::ranking` (`calcular_nota`, `aplicar_penalidade_continue`): Z-score → nota
      0..10 (`z=0` → 5.0, satura em 10.0/1.0 a partir de `±3σ`), penalidade de continue (corta
      90%), divisão por zero tratada (sem `mu`/`sigma`, ou `sigma = 0`) devolvendo uma nota base
      neutra (5.0) em vez de propagar `NaN`/`inf`. 10 testes unitários (`cargo test`, lógica pura
      sem I/O) cobrindo cada caso, incluindo o valor `z=1.5 → nota 7.5` conferido manualmente em
      SQL antes de escrever o Rust (ver validação abaixo).
- [x] 3.11 `GET /api/rankings/global` (`routes/rankings.rs`): Ranking Global Oficial — soma das
      notas máximas (Hi-Score) só dos **5 jogos mais jogados** (por volume de runs válidas, não
      pelos 5 com as melhores notas) de cada jogador. CTE `jogos_por_usuario` (`MAX(nota_normalizada)`
      + `COUNT(id)` por jogador/jogo) → `ROW_NUMBER() OVER (PARTITION BY jogador_id ORDER BY
      total_runs DESC, maior_nota DESC, jogo_id)` (os dois últimos critérios só para desempate
      determinístico — `ROW_NUMBER()` sem ordenação totalmente determinística pode variar entre
      chamadas quando dois jogos têm o mesmo número de runs) → filtro `rn <= 5` → `SUM`/`RANK()`.
      Jogador com menos de 5 jogos soma normalmente o que tem, sem erro nem necessidade de
      preencher posições vazias.

### Validação feita nesta tarefa (sqlx compile-time check + offline cache)
`sqlx::query_as!` verifica a query **no momento da compilação**, direto contra um banco — não dá
para só "ler o SQL e confiar". Por isso:
1. Subi um Postgres 16 + pgvector temporário via Docker, com a migration real aplicada.
2. Compilei (`cargo check`/`build`) com `DATABASE_URL` apontando pra ele — a macro validou os tipos
   de `SELECT id, steam_app_id, nome, criado_em FROM public.jogos` contra as colunas reais
   (`uuid`, `int4`, `varchar`, `timestamptz`).
3. Rodei o binário de verdade: `GET /api/jogos` devolveu os dois jogos seedados em JSON 200;
   `GET /health` 200; renomear a tabela `jogos` em runtime e bater em `/api/jogos` deu 500 limpo
   sem matar o processo (`/health` respondeu normalmente depois).
4. Instalei o `sqlx-cli` e rodei `cargo sqlx prepare`, gerando `.sqlx/*.json` (**comitado no git**).
   Confirmei que `cargo check` compila limpo **sem** `DATABASE_URL` e **sem** o container Docker
   rodando — é esse cache offline que faz `cargo check` "continuar compilando perfeitamente" no
   dia a dia de quem não tem um Postgres local de pé. Rodar `cargo sqlx prepare` de novo só é
   necessário quando uma query mudar ou uma nova for adicionada.
5. Removi o container de teste ao final; nenhum `.env` foi comitado (a connection string real do
   Supabase deve ser configurada localmente por cada ambiente, nunca no git).

### Validação feita nesta tarefa (ranking + antifraude de runs)
Antes de escrever uma linha de Rust, validei a lógica em SQL puro contra um Postgres 16 +
pgvector real (as três migrations aplicadas em sequência): seedei 3 runs válidas históricas
(scores 100000/150000/50000 → `mu=100000, sigma=50000`), simulei `/runs/start` + submissão de
score 175000 via `BEGIN`/`UPDATE`/`COMMIT` manual e confirmei `nota = 7.5` (bate com `z = 1.5`),
e confirmei que `perfis_jogadores.pontuacao_global_normalizada` foi atualizada corretamente.
Só depois disso escrevi `routes/runs.rs` e `services/ranking.rs`. Depois de implementado:
- `cargo check`/`cargo test` com `DATABASE_URL` real: 10/10 testes unitários de `ranking.rs`
  passando, compile-time check das 9 queries novas contra o schema real.
- Corrigi `nota_normalizada` de `NUMERIC(4,2)` para `DOUBLE PRECISION` na migration (ver 1c.2)
  porque o sqlx pediria a feature `bigdecimal`/`rust_decimal` só para bindar um `f64`.
- Rodei o binário de verdade contra o Postgres de teste e testei ponta a ponta via `curl`:
  `/runs/start` decrementando fichas de verdade (5→4, confirmado via `SELECT`); submissão sem
  histórico anterior caindo na nota base 5.0 (tabela `runs` tinha sido recriada vazia ao corrigir
  o tipo da coluna, então esse caso de "jogo novo" aconteceu de propósito); segunda submissão sem
  ficha ativa → `400`; `continues_usados=1` cortando a nota para `0.5` (`5.0 × 0.1`); janela
  expirada (`server_started_at` adiantado 2h manualmente) → `409` sem devolver a ficha.
- `cargo sqlx prepare` rodado ao final; confirmei `cargo check`/`cargo test` **offline** (sem
  `DATABASE_URL`, sem Docker) depois de um `cargo clean` do pacote — 10/10 testes e compilação
  limpa usando só o cache `.sqlx/` atualizado.

### Validação feita nesta tarefa (regra dos "5 jogos mais jogados")
A regra "soma dos 5 jogos mais jogados" é fácil de implementar errado como "soma das 5 melhores
notas" — são a mesma query até os dados ficarem desalinhados. Para provar que o critério certo
estava sendo usado (volume de runs, não nota), segui o mesmo processo de validar em SQL puro
antes do Rust:
1. Semeei um jogador com 7 jogos: 5 jogados muito (6 a 10 runs cada, nota 5.0) e 2 jogados pouco
   (1 e 2 runs, nota 9.9 cada — deliberadamente a nota mais alta de todos). Se a query escolhesse
   por nota em vez de por volume, o resultado incluiria os dois jogos de nota 9.9.
2. A CTE + `ROW_NUMBER() OVER (PARTITION BY jogador_id ORDER BY total_runs DESC, ...)` + filtro
   `rn <= 5` excluiu corretamente os dois jogos de nota 9.9 (os menos jogados) e somou só os 5
   jogos de nota 5.0 → `soma_top5_notas = 25.0`, confirmando que o critério é mesmo volume de
   runs, não nota.
3. Segundo jogador com só 2 jogos: somou `13.7` (a soma literal dos dois) sem erro — confirma o
   caso defensivo de "jogou menos de 5 jogos" pedido na tarefa.
4. Rodei a mesma verificação via `curl` contra o binário real (`GET /api/rankings/global`) e o
   JSON devolvido bateu exatamente com os números calculados em SQL puro.
5. `cargo sqlx prepare` + confirmação de `cargo check`/`cargo test` offline (sem `DATABASE_URL`,
   sem Docker, depois de `cargo clean`) ao final, igual às tarefas anteriores.

### Validação feita nesta tarefa (middleware JWT)
Nenhuma query SQL mudou de texto nesta tarefa (só a origem do `usuario_id` em Rust), então o
cache `.sqlx/` já existente continuou válido — confirmei isso rodando `cargo sqlx prepare
--check` contra um Postgres real com as 3 migrations aplicadas (saída limpa, sem diffs), em vez
de simplesmente assumir que não precisava regenerar.

Para testar o middleware de ponta a ponta com um JWT de verdade (não só os 7 testes unitários
puros), criei um `examples/mint_token.rs` temporário (usando a própria `jsonwebtoken` já
adicionada) para gerar um token HS256 válido com um `sub` escolhido, subi o servidor real com
`SUPABASE_JWT_SECRET` configurada e testei via `curl`:
- `GET /api/jogos` sem token → `200` (continua pública, o middleware não entrou no caminho dela).
- `POST /api/runs/start` sem header `Authorization` → `401 "cabeçalho Authorization ausente"`.
- Mesmo endpoint com um token assinado com um segredo diferente → `401 "token inválido ou
  expirado"`.
- Com um token válido → `201`, e o `jogador_id` gravado em `public.runs` bate exatamente com o
  `sub` do token (`SELECT` direto no Postgres).
- **Tentativa de ataque explícita**: enviei um `usuario_id` de outro jogador no corpo JSON
  junto com um token válido de um jogador diferente — a run foi gravada no jogador do token,
  não no UUID forjado no corpo (que nem existe mais como campo do struct, então não tem como
  nem ser lido). Removi o `examples/mint_token.rs` depois de validar — não faz parte do binário
  final, só foi uma ferramenta de teste.

## 4. Pendências conhecidas de schema (independentes do Axum)
- [ ] 4.1 Tabela/colunas para o **hash SHA-256** e **canal** (`draft`/`stable`) de cada
      `perfil_versoes` — a Spec 001 pede isso explicitamente para o ciclo de publicação; não veio
      na migration inicial porque aquela tarefa pediu só o schema base.
- [x] 4.2 ~~Tabela de `runs`~~ — criada em 1c (`20261008140000_add_runs.sql`), junto com as rotas
      3.7, porque uma não fazia sentido sem a outra.
- [ ] 4.3 Expiração/limpeza de `salas_online` (linhas de presença órfãs quando o heartbeat para sem
      `DELETE` explícito) — hoje depende do Client/Realtime chamar a saída corretamente.
- [ ] 4.4 `services::ranking` hoje calcula `mu`/`sigma` sobre **todas** as runs válidas já
      registradas para o jogo, sem recorte de temporada — a Spec 002 pede o Z-score calculado
      *dentro de uma temporada*. Simplificação deliberada desta tarefa (não havia pedido explícito
      para modelar temporadas, e isso exige sua própria entidade com início/fim e lógica de
      "temporada atual"); documentando para não passar como se já estivesse correto por completo.
- [x] ~~4.5~~ Resolvido em 3.9: `usuario_id` agora vem do JWT (`Extension<Uuid>`), não do
      payload — o campo foi removido dos structs, não só "parou de ser lido".
- [ ] 4.5b `GET /api/jogos` e `GET /api/rankings/global` continuam públicas de propósito (não
      precisam de sessão para ler dados agregados/catálogo); se isso mudar no futuro (ex.: um
      ranking "só para assinantes"), o mesmo `middleware::auth::exigir_usuario_autenticado`
      pode ser aplicado via `route_layer` nelas também.
- [ ] 4.5c Suporte a RS256 (JWKS) para projetos Supabase que assinam com chave assimétrica —
      hoje só HS256/segredo compartilhado é suportado (ver nota em 3.9).
- [ ] 4.6 Índice de ranking: `GET /ranking/:jogo_id` (3.8) ainda não existe; hoje só é possível ver
      `pontuacao_global_normalizada` lendo `perfis_jogadores` diretamente, sem ordenação/paginação.
- [ ] 4.7 "Índice global do jogador" (Spec 002: média/soma dos top-K jogos, com *shrinkage* para
      quem jogou poucos jogos) foi simplificado para "média de todas as runs válidas do jogador,
      em todos os jogos" — sem top-K, sem shrinkage. O pedido desta tarefa ("some ou tire a média
      das performances") permitia essa simplificação; top-K/shrinkage ficam para quando o ranking
      precisar comparar jogadores que jogam números bem diferentes de jogos.

## 5. Pendências conhecidas de `robotx-voice`
- [ ] 5.1 Round-trip completo não testado com credenciais reais (Whisper → embedding → RPC com um
      vetor de verdade → chat → TTS). O que foi validado é: type-check/lint reais, e o caminho de
      erro de rede/configuração (ver 2.1). A chamada `supabase.rpc("buscar_conhecimento_robotx",
      { param_embedding: embedding, ... })` passa o embedding como array JS puro — é o padrão
      documentado do Supabase para `vector`, mas só fica 100% confirmado testando contra um
      Postgres com pgvector de verdade e uma `OPENAI_API_KEY` válida.
- [ ] 5.2 Nenhum rate limit / proteção contra abuso (um jogador podendo disparar `robotx-voice`
      em loop e gerar custo de API sem controle).
- [ ] 5.3 `shmupx-client` ainda não chama esta função (TASKS.md do client, item 4.5): o `.wav`
      gerado por `capture/audio.rs` hoje só fica salvo localmente, nada o envia.
- [ ] 5.4 Sem teste automatizado (unitário/integração) para o pipeline da Edge Function.

## Firestore (ADR 005) — fichas e gameplays
- [x] `middleware/firebase.rs`: valida ID token do Firebase (RS256, JWKS do Google em cache, `aud`/`iss` do projeto, `exp`/`iat`/`auth_time`, `sub`). Revogação não é checada (exigiria Admin SDK).
- [x] `firestore/`: cliente REST com conta de serviço (`GOOGLE_APPLICATION_CREDENTIALS`, fluxo JWT bearer) ou emulador (`FIRESTORE_EMULATOR_HOST`). Leituras transacionais via `:batchGet` (o `GET ?transaction=` trava no emulador).
- [x] `services/gameplays.rs`: `POST /api/runs/start` debita 1 ficha em `wallets/{uid}` (carteira nasce com 50, lançamentos em `entries`) e abre `gameplays/{runId}` em `recording`; `POST /api/runs` recebe `run_id`, valida dono, jogo e janela, grava `result` e muda para `needs_review` (reenvio idêntico é idempotente).
- [x] Postgres opcional: `/api/jogos` e `/api/rankings` só sobem com `DATABASE_URL`. Ponte `firebase_uid` no Postgres descartada antes de ser aplicada.
- [x] Testes de integração no emulador: `npx firebase-tools emulators:exec --only firestore --project demo-shmupx "cd ../../shmupx-backend && cargo test -- --ignored --test-threads=1"` (rodar em `shmupx-infra/firebase`).
- [ ] Publicar `games/earthion` (`published: true`, `maxDurationSeconds`) em produção — sem ele nenhuma run inicia.
- [ ] Aprovação de `needs_review`, nota normalizada e publicação dos rankings no Firestore (preservar a regra dos 5 jogos mais jogados).
- [ ] Edge Function `robotx-voice` ainda valida JWT do Supabase.