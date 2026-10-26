-- ShmupX — ponte de identidade Firebase Authentication -> perfis_jogadores (ADR 005, etapa 1)
--
-- O login do cliente passou a ser Google via Firebase Auth. Até fichas e runs migrarem para o
-- Firestore, o Postgres continua dono do saldo e das runs, mas o jogador é identificado pelo UID
-- do Firebase. O perfil é criado pelo backend no primeiro acesso autenticado.
-- Idempotente, como as migrations anteriores.

-- O id deixa de depender de auth.users (jogadores Firebase não existem no Supabase Auth) e
-- passa a ser gerado aqui. O trigger handle_new_user continua cobrindo contas Supabase legadas.
alter table public.perfis_jogadores drop constraint if exists perfis_jogadores_id_fkey;
alter table public.perfis_jogadores alter column id set default extensions.uuid_generate_v4();

-- UID do Firebase (até 128 caracteres). Único: um perfil por conta.
alter table public.perfis_jogadores add column if not exists firebase_uid varchar(128);
create unique index if not exists perfis_jogadores_firebase_uid_key
  on public.perfis_jogadores (firebase_uid);

-- Saldo inicial maior para novos jogadores. O backend também envia o valor explicitamente.
alter table public.perfis_jogadores alter column fichas_disponiveis set default 50;

comment on column public.perfis_jogadores.firebase_uid is
  'UID do Firebase Authentication. Nulo apenas em perfis legados do Supabase Auth.';
