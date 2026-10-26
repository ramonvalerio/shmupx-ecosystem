-- ShmupX — identidade de sala (necessária para resolver sala_id -> jogo_id no servidor)
-- Relacionado: docs/adr/003-agente-robotx-e-salas.md
--
-- Lacuna que esta migration corrige: a migration anterior (20261008120000) só modelou
-- `salas_online` como presença (jogo_id + jogador_id), sem uma entidade de sala com ID
-- próprio. A Edge Function `robotx-voice` recebe `sala_id` do cliente e precisa resolver o
-- `jogo_id` real a partir dela NO SERVIDOR — nunca confiando no `jogo_id` que o cliente possa
-- enviar — para impedir que o RAG de uma sala vaze conhecimento de outro jogo (ADR 003). Sem
-- esta tabela, não havia como fazer essa resolução.

create table if not exists public.salas (
  id uuid primary key default extensions.uuid_generate_v4(),
  jogo_id uuid not null references public.jogos (id) on delete cascade,
  criado_em timestamptz not null default now()
);

comment on table public.salas is
  'Identidade de uma sala (um canal Supabase Realtime). A Edge Function robotx-voice resolve '
  'sala_id -> jogo_id a partir desta tabela, nunca a partir de um valor enviado pelo cliente.';

-- `salas_online` (presença) passa a referenciar a sala, não só o jogo diretamente — mantém a
-- coluna `jogo_id` (não é redundante: permite consultas de presença por jogo sem join).
alter table public.salas_online
  add column if not exists sala_id uuid references public.salas (id) on delete cascade;

create index if not exists idx_salas_jogo_id on public.salas (jogo_id);
create index if not exists idx_salas_online_sala_id on public.salas_online (sala_id);

alter table public.salas enable row level security;

-- Leitura pública (lista de salas ativas no portal/cliente); escrita só via service_role.
drop policy if exists salas_leitura_publica on public.salas;
create policy salas_leitura_publica
  on public.salas for select
  to anon, authenticated
  using (true);
