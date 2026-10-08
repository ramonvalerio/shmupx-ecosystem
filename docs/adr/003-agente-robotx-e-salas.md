# ADR 003 — Agente RobotX, voz por hotkey e isolamento de RAG por sala

- **Status:** Aceito
- **Data:** 2026-10-08

## Contexto

O RobotX AI é o tutor/agente da plataforma. Durante o jogo o jogador não pode usar teclado/mouse. O agente deve responder apenas sobre o jogo da sala atual, sem vazar conhecimento de outros jogos.

## Decisão

### Voz por hotkey no controle
- O módulo Rust `input` escuta um botão configurável do joystick/controle em segundo plano (global, independente do foco da janela).
- Pressionar = abre captura de microfone (push-to-talk); soltar = encerra e envia o áudio ao backend. O Client emite eventos Tauri para a UI mostrar o estado.
- O áudio não é gravado em disco por padrão; trafega por Edge Function para transcrição + resposta.

### Salas
- Salas em tempo real via **Supabase Realtime** (um canal por sala), cada sala vinculada a um `steam_app_id`.
- Entrar na sala define o **contexto de jogo** da sessão.

### Isolamento de RAG por sala
- Base de conhecimento em **pgvector**, com cada chunk marcado por `game_id`.
- A Edge Function do RobotX recebe `room_id`, resolve o `game_id` **no servidor** (nunca confia no cliente) e executa a busca vetorial sempre com filtro `game_id = :resolvido`, reforçado por **RLS**.
- O *system prompt* mantém a personalidade padrão da plataforma e restringe respostas ao contexto recuperado do jogo; sem contexto relevante, o agente diz que não sabe.

## Consequências

- (+) Vazamento entre jogos impedido por construção (filtro server-side + RLS).
- (+) Jogador usa só o controle.
- (−) Latência de rede no fluxo de voz; mitigação com resposta em streaming.
