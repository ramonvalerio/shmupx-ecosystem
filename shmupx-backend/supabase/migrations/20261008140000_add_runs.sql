-- ShmupX — histórico de runs, ficha ativa e antifraude de janela de tempo (Spec 002)
-- Relacionado: docs/specs/002-economia-fichas-e-ranking.md
--
-- Pré-requisito para a rota POST /api/runs (submissão de score): sem uma tabela que registre
-- a "ficha ativa" aberta em POST /api/runs/start, não há nada contra o que validar o timestamp
-- de encerramento nem o uso de continues. As duas rotas nascem juntas nesta migration porque
-- uma não faz sentido sem a outra.

-- Cada jogo define sua própria janela máxima de run (45–60 min, Spec 002); default de 60 min
-- para jogos que ainda não tiveram essa configuração ajustada manualmente.
alter table public.jogos
  add column if not exists max_duration_segundos integer not null default 3600
  check (max_duration_segundos between 1 and 7200);

create table if not exists public.runs (
  id uuid primary key default extensions.uuid_generate_v4(),
  jogador_id uuid not null references public.perfis_jogadores (id) on delete cascade,
  jogo_id uuid not null references public.jogos (id) on delete cascade,
  server_started_at timestamptz not null default now(),
  max_duration_segundos integer not null,
  status text not null default 'ativa' check (status in ('ativa', 'valida', 'invalida', 'expirada')),
  score_bruto bigint,
  vidas_perdidas integer,
  continues_usados integer,
  nota_normalizada double precision,
  submetida_em timestamptz,
  criado_em timestamptz not null default now()
);

comment on table public.runs is
  'Ficha ativa (status=ativa, criada por POST /api/runs/start) e o resultado submetido '
  '(score_bruto, nota_normalizada) depois da análise ONNX no Client (POST /api/runs). O timer '
  'seguro do Client é a fonte soberana do corte, mas server_started_at + max_duration_segundos '
  'é a fonte soberana da validação antifraude no backend.';

create index if not exists idx_runs_jogador_jogo_status on public.runs (jogador_id, jogo_id, status);
create index if not exists idx_runs_jogo_id_status on public.runs (jogo_id, status);

alter table public.runs enable row level security;

-- Só SELECT da própria run via PostgREST/anon-authenticated. Nenhuma policy de INSERT/UPDATE
-- para essas roles de propósito: todo write em `runs` passa pela API Axum (que aplica as
-- validações antifraude em Rust antes de gravar) — nunca direto do cliente.
drop policy if exists runs_select_proprio on public.runs;
create policy runs_select_proprio
  on public.runs for select
  to authenticated
  using (auth.uid() = jogador_id);
