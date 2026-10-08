# ADR 001 — Arquitetura monorepo orientada a DDD

- **Status:** Aceito
- **Data:** 2026-10-08

## Contexto

A plataforma ShmupX foi dividida historicamente em vários repositórios independentes (.NET, React, Terraform). Para um desenvolvimento solo, isso gera sobrecarga de sincronização de contratos e versões. A reescrita parte do zero com Tauri v2 (Rust + React/TypeScript) e Supabase.

## Decisão

Adotar um **monorepo** com responsabilidades isoladas por pasta e linguagem ubíqua compartilhada via `docs/`.

| Pasta | Papel | Stack |
|---|---|---|
| `shmupx-client/` | App desktop do jogador | Tauri v2, Rust, React |
| `shmupx-lab/` | Backoffice local do admin | Tauri v2, Rust, React |
| `shmupx-frontend/` | Portal web público (rankings) | React/Next.js |
| `shmupx-backend/` | API e Supabase (migrations, Edge Functions) | PostgreSQL, pgvector |
| `shmupx-infra/` | Infraestrutura como código | Terraform |
| `docs/` | Specs e ADRs | Markdown |

> **Estado atual:** `shmupx-client/` e `shmupx-lab/` têm esqueletos Tauri v2 compilando. `shmupx-frontend/`, `shmupx-backend/` e `shmupx-infra/` são apenas placeholders. Os projetos legados vivem fora deste repositório.

### Bounded Contexts

1. **Sessão de Jogo (Client)** — salas em tempo real, detecção de jogo Steam, início da run. Módulo Rust: `steam`.
2. **Captura & Antifraude (Client)** — FFmpeg, gravação, timer seguro, corte por estouro. Módulo: `capture`.
3. **Assistente RobotX (Client + Backend)** — voz por hotkey, RAG isolado por sala. Módulo: `input` + Edge Function. Ver ADR 003.
4. **Análise & Perfis (Lab → Client)** — mapeamento de regiões, perfis versionados LCD/CRT, inferência ONNX. Ver ADR 002.
5. **Economia & Ranking (Backend + Frontend)** — fichas digitais, runs válidas, normalização Z/percentil. Ver spec 002.

### Regras de dependência

- Contextos conversam por **contratos** (JSON versionado / eventos), nunca por acesso direto ao banco de outro contexto.
- Cada módulo Rust expõe *traits* (portas) no domínio; implementações dependentes de SO/processo ficam na borda.
- O frontend React dos apps desktop só orquestra UI; regras sensíveis (timer, corte, análise) ficam em Rust.

## Consequências

- (+) Um único histórico, refatorações atômicas entre contratos.
- (+) Specs/ADRs versionados junto ao código.
- (−) CI precisa filtrar por caminho para evitar builds desnecessários.
