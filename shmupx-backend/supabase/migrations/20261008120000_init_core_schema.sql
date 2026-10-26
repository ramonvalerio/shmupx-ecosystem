-- ShmupX — schema inicial (Gamificação, Comunidade, Tutoria)
-- Relacionado: docs/adr/001, docs/adr/002, docs/adr/003, docs/specs/001, docs/specs/002
--
-- Convenções:
--   * Todas as instruções são idempotentes (IF NOT EXISTS / CREATE OR REPLACE / DROP ... IF EXISTS
--     antes de recriar), para que esta migration possa ser reaplicada sem erro.
--   * Extensões vivem no schema `extensions` (padrão Supabase), não em `public`.
--   * RLS é habilitado em toda tabela com dados de jogador; tabelas de catálogo (jogos,
--     perfil_versoes) ficam com leitura pública e escrita restrita a service_role (bypassa RLS).

-- ============================================================================
-- 1. Extensões
-- ============================================================================

create schema if not exists extensions;

-- pgvector: embeddings do RAG do RobotX (ver docs/adr/003).
create extension if not exists vector with schema extensions;

-- uuid-ossp: uuid_generate_v4() usado como default de chave primária nas tabelas abaixo.
create extension if not exists "uuid-ossp" with schema extensions;

-- ============================================================================
-- 2. Jogos e versões de perfil de análise (Spec 001)
-- ============================================================================

create table if not exists public.jogos (
  id uuid primary key default extensions.uuid_generate_v4(),
  steam_app_id integer not null unique,
  nome varchar(255) not null,
  criado_em timestamptz not null default now()
);

comment on table public.jogos is
  'Catálogo de shmups suportados pela plataforma, identificados pelo app_id da Steam.';

-- Perfis publicados são imutáveis (Spec 001): a combinação jogo+versão+variante nunca se repete;
-- uma correção sempre gera uma nova versão.
create table if not exists public.perfil_versoes (
  id uuid primary key default extensions.uuid_generate_v4(),
  jogo_id uuid not null references public.jogos (id) on delete cascade,
  versao varchar(32) not null,
  variante varchar(8) not null check (variante in ('LCD', 'CRT')),
  config_normalizada_json jsonb not null,
  criado_em timestamptz not null default now(),
  constraint perfil_versoes_jogo_versao_variante_key unique (jogo_id, versao, variante)
);

comment on table public.perfil_versoes is
  'Releases imutáveis de AnalysisProfile publicadas pelo Lab (regiões normalizadas + referência de modelo).';
comment on column public.perfil_versoes.config_normalizada_json is
  'AnalysisProfile.regions normalizado 0..1, no formato consumido pelo motor ONNX do Client.';

-- ============================================================================
-- 3. Base de conhecimento vetorial do RobotX (RAG isolado por jogo — ADR 003)
-- ============================================================================

create table if not exists public.robotx_conhecimento (
  id uuid primary key default extensions.uuid_generate_v4(),
  jogo_id uuid not null references public.jogos (id) on delete cascade,
  conteudo text not null,
  embedding extensions.vector(1536) not null,
  criado_em timestamptz not null default now()
);

comment on table public.robotx_conhecimento is
  'Chunks de wikis/guias por jogo para RAG do RobotX. Nunca é lido diretamente pelo cliente: '
  'só via buscar_conhecimento_robotx(), que impõe o filtro jogo_id no servidor (ADR 003).';

-- ============================================================================
-- 4. Busca vetorial isolada por sala (RPC usada pela Edge Function do RobotX)
-- ============================================================================

-- SECURITY DEFINER: a Edge Function resolve `room_id -> jogo_id` no servidor e chama esta
-- função; o filtro `jogo_id = param_jogo_id` abaixo é o que impede o RobotX de uma sala
-- enxergar conhecimento de outro jogo, mesmo que o cliente tente manipular o parâmetro
-- (a Edge Function nunca aceita jogo_id vindo do cliente, apenas room_id).
create or replace function public.buscar_conhecimento_robotx(
  param_jogo_id uuid,
  param_embedding extensions.vector(1536),
  param_limite integer default 5
)
returns table (
  id uuid,
  conteudo text,
  similaridade double precision
)
language sql
stable
security definer
set search_path = public, extensions
as $$
  select
    rc.id,
    rc.conteudo,
    1 - (rc.embedding <=> param_embedding) as similaridade
  from public.robotx_conhecimento as rc
  where rc.jogo_id = param_jogo_id
  order by rc.embedding <=> param_embedding
  limit greatest(param_limite, 1);
$$;

comment on function public.buscar_conhecimento_robotx is
  'Busca por similaridade de cosseno (1 - distância <=>) restrita a um único jogo. '
  'Reforça em SQL o isolamento de contexto que a Edge Function já impõe ao resolver '
  'room_id -> jogo_id no servidor (nunca confia no valor vindo do cliente).';

revoke all on function public.buscar_conhecimento_robotx(uuid, extensions.vector, integer) from public;
grant execute on function public.buscar_conhecimento_robotx(uuid, extensions.vector, integer) to service_role;

-- ============================================================================
-- 5. Economia de fichas e ranking (Spec 002)
-- ============================================================================

create table if not exists public.perfis_jogadores (
  id uuid primary key references auth.users (id) on delete cascade,
  fichas_disponiveis integer not null default 5 check (fichas_disponiveis >= 0),
  pontuacao_global_normalizada numeric(10, 2) not null default 0.00,
  criado_em timestamptz not null default now(),
  atualizado_em timestamptz not null default now()
);

comment on table public.perfis_jogadores is
  'Extensão de auth.users com saldo de fichas digitais e índice global de ranking (Spec 002).';

-- Mantém atualizado_em em dia a cada UPDATE, sem depender do app lembrar de setá-la.
create or replace function public.set_atualizado_em()
returns trigger
language plpgsql
as $$
begin
  new.atualizado_em = now();
  return new;
end;
$$;

drop trigger if exists trg_perfis_jogadores_atualizado_em on public.perfis_jogadores;
create trigger trg_perfis_jogadores_atualizado_em
  before update on public.perfis_jogadores
  for each row
  execute function public.set_atualizado_em();

-- Cria automaticamente o perfil (com as 5 fichas iniciais) quando um novo usuário se registra.
create or replace function public.handle_new_user()
returns trigger
language plpgsql
security definer
set search_path = public
as $$
begin
  insert into public.perfis_jogadores (id)
  values (new.id)
  on conflict (id) do nothing;
  return new;
end;
$$;

drop trigger if exists trg_auth_users_criar_perfil on auth.users;
create trigger trg_auth_users_criar_perfil
  after insert on auth.users
  for each row
  execute function public.handle_new_user();

-- Presença em tempo real: uma linha por jogador atualmente numa sala (sala = contexto de
-- jogo, ADR 003). O canal Realtime em si vive no Supabase Realtime; esta tabela é o registro
-- persistido usado para reconciliação/analytics quando o heartbeat falha.
create table if not exists public.salas_online (
  id uuid primary key default extensions.uuid_generate_v4(),
  jogo_id uuid not null references public.jogos (id) on delete cascade,
  jogador_id uuid not null references public.perfis_jogadores (id) on delete cascade,
  entrou_em timestamptz not null default now(),
  ultimo_heartbeat timestamptz not null default now(),
  constraint salas_online_jogo_jogador_key unique (jogo_id, jogador_id)
);

comment on table public.salas_online is
  'Presença ativa por sala/jogo. Uma linha por jogador online; removida ao sair ou expirar o heartbeat.';

-- ============================================================================
-- 6. Índices
-- ============================================================================

create index if not exists idx_perfil_versoes_jogo_id on public.perfil_versoes (jogo_id);
create index if not exists idx_robotx_conhecimento_jogo_id on public.robotx_conhecimento (jogo_id);
create index if not exists idx_salas_online_jogo_id on public.salas_online (jogo_id);
create index if not exists idx_salas_online_jogador_id on public.salas_online (jogador_id);

-- Índice HNSW para a busca vetorial: deliberadamente NÃO criado agora (custo de build é alto
-- e o ganho só importa com muitos registros). Quando `robotx_conhecimento` crescer o bastante
-- para a sequential scan do `<=>` pesar, rodar manualmente (fora desta migration, em janela
-- de manutenção):
--
--   create index concurrently if not exists idx_robotx_conhecimento_embedding_hnsw
--     on public.robotx_conhecimento
--     using hnsw (embedding extensions.vector_cosine_ops);
--
-- O opclass `vector_cosine_ops` casa com o operador `<=>` usado em buscar_conhecimento_robotx.

-- ============================================================================
-- 7. Row Level Security
-- ============================================================================

alter table public.jogos enable row level security;
alter table public.perfil_versoes enable row level security;
alter table public.robotx_conhecimento enable row level security;
alter table public.perfis_jogadores enable row level security;
alter table public.salas_online enable row level security;

-- jogos / perfil_versoes: catálogo público de leitura (portal web, Client); escrita só via
-- service_role (Lab publica usando a service key, que sempre bypassa RLS).
drop policy if exists jogos_leitura_publica on public.jogos;
create policy jogos_leitura_publica
  on public.jogos for select
  to anon, authenticated
  using (true);

drop policy if exists perfil_versoes_leitura_publica on public.perfil_versoes;
create policy perfil_versoes_leitura_publica
  on public.perfil_versoes for select
  to anon, authenticated
  using (true);

-- robotx_conhecimento: nenhuma policy de select para anon/authenticated — de propósito.
-- O único caminho de leitura é a função SECURITY DEFINER buscar_conhecimento_robotx(),
-- chamada pela Edge Function. Isso é o que torna o isolamento por jogo à prova de bypass
-- via PostgREST direto (ver ADR 003).

-- perfis_jogadores: cada jogador só vê/edita a própria linha.
drop policy if exists perfis_jogadores_select_proprio on public.perfis_jogadores;
create policy perfis_jogadores_select_proprio
  on public.perfis_jogadores for select
  to authenticated
  using (auth.uid() = id);

drop policy if exists perfis_jogadores_update_proprio on public.perfis_jogadores;
create policy perfis_jogadores_update_proprio
  on public.perfis_jogadores for update
  to authenticated
  using (auth.uid() = id)
  with check (auth.uid() = id);

-- salas_online: presença é visível para qualquer autenticado (lista de jogadores online na
-- sala); cada jogador só cria/remove/atualiza a própria presença.
drop policy if exists salas_online_select_autenticado on public.salas_online;
create policy salas_online_select_autenticado
  on public.salas_online for select
  to authenticated
  using (true);

drop policy if exists salas_online_insert_proprio on public.salas_online;
create policy salas_online_insert_proprio
  on public.salas_online for insert
  to authenticated
  with check (auth.uid() = jogador_id);

drop policy if exists salas_online_update_proprio on public.salas_online;
create policy salas_online_update_proprio
  on public.salas_online for update
  to authenticated
  using (auth.uid() = jogador_id)
  with check (auth.uid() = jogador_id);

drop policy if exists salas_online_delete_proprio on public.salas_online;
create policy salas_online_delete_proprio
  on public.salas_online for delete
  to authenticated
  using (auth.uid() = jogador_id);
