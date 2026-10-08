# ShmupX Client — Tarefas de implementação (Rust)

Ordem sequencial. Cada item deve terminar com `cargo check` e testes unitários passando.
Contexto: `docs/adr/001..003` e `docs/specs/002`.

## 0. Fundação
- [ ] 0.1 Definir tipo de erro comum (`thiserror`) e `Result` do crate em `src-tauri/src/error.rs`.
- [ ] 0.2 Adicionar `tokio` (rt, process, time, sync) e `tracing`; inicializar runtime/estado global via `tauri::Builder::manage`.
- [ ] 0.3 Definir eventos Tauri tipados (`run://tick`, `run://expired`, `input://ptt`) e expô-los ao frontend.

## 1. Módulo `steam` (detecção de processos)
- [x] 1.1a Adicionar `sysinfo`; implementar `is_steam_running()` em `steam/detector.rs` (varredura por nome de processo `steam.exe`/`steam`).
- [ ] 1.1b Varredura completa de processos para o `GameDetector` (depende de 1.2–1.4).
- [ ] 1.2 Localizar a instalação da Steam (registro do Windows) e ler `libraryfolders.vdf`.
- [ ] 1.3 Ler `appmanifest_*.acf` para mapear pasta de instalação → `app_id` + nome.
- [ ] 1.4 Implementar `GameDetector` casando o executável do processo com o `installdir` do manifest.
- [x] 1.5a Stub de monitoramento: `steam/commands.rs::spawn_launch_monitor` faz polling (2s, timeout 60s) por qualquer processo novo após `steam://run/<app_id>` e emite `steam://game-launched` / `steam://game-launch-timeout`. **Heurística provisória** (qualquer processo novo, não necessariamente o jogo) — trocar por casamento exato via manifesto quando 1.2–1.4 existirem; é o ponto de partida para disparar o FFmpeg automaticamente no futuro.
- [ ] 1.5b Polling leve (1–2 s) emitindo `steam://game-changed` apenas em mudança real de jogo (depende de 1.2–1.4).
- [ ] 1.6 Testes com fixtures de `.acf` e lista de processos fake.

### Comandos Tauri expostos (steam/commands.rs)
- [x] `inicializar_jogo_steam(steam_app_id)`: valida `app_id` e Steam em execução, dispara `steam://run/<app_id>` via abridor de URI do SO (Windows/macOS/Linux) e agenda o monitor de lançamento em background.

## 2. Módulo `capture` — timer seguro
- [ ] 2.1 Implementar máquina de estados de `RunTimer` (Idle → Running → Expired/Finished) com `Instant`.
- [ ] 2.2 Task assíncrona de tick (1 s) emitindo `run://tick` com tempo restante.
- [x] 2.3a Timer de corte via `tokio::spawn` + `tokio::time::sleep` (duração fixa de 5 min para testes rápidos; `TEST_RUN_DURATION` em `capture/run_timer.rs` — substituir por `max_duration` do servidor quando a integração com o backend existir).
- [ ] 2.3b Aceitar `max_duration` vindo do servidor (45–60 min) e clamp no limite superior (depende do backend).
- [x] 2.4 Ao expirar, matar o processo FFmpeg (`child.kill().await`) e emitir `run://expired` com o `jogo_id` (`capture/commands.rs::spawn_safe_timer`).
- [ ] 2.5 Testes com `tokio::time::pause` cobrindo expiração, finalização antecipada e dupla finalização.

## 3. Módulo `capture` — FFmpeg
- [x] 3.1a Verificação de disponibilidade do binário no PATH (`ffmpeg -version`) via `capture::ffmpeg::is_ffmpeg_available`.
- [ ] 3.1b Resolver via sidecar do Tauri (hoje depende do PATH do sistema) e validar versão mínima.
- [ ] 3.2 Montar a linha de comando de captura real (gdigrab/ddagrab + áudio WASAPI) a partir de `RecordingConfig` — hoje `spawn_ffmpeg` usa um padrão de teste (`testsrc`) só para validar o ciclo de vida do processo.
- [x] 3.3 Spawn assíncrono com `tokio::process::Command`, PID armazenado em estado global (`capture::state::RunState`, `tauri::State` com `Arc<Mutex<Option<RunHandle>>>`).
- [x] 3.4 Parada graciosa (`q` no stdin + `shutdown`) com fallback para `kill` após timeout de 3s (`capture::ffmpeg::graceful_stop`, usado pelo comando `finalizar_corrida_manual`).
- [ ] 3.5 Saída RTMP opcional (tee muxer) compartilhando o mesmo processo — `RecordingConfig.rtmp_url` já existe, falta o muxer `tee` real.
- [ ] 3.6 Marcar a run como `invalid` no backend se o processo morrer fora do esperado (hoje só é tratado localmente).
- [ ] 3.7 Extrair metadados do arquivo final (duração, hash SHA-256) para o payload de submissão.

### Comandos Tauri expostos (capture/commands.rs)
- [x] `iniciar_corrida_safe(jogo_id)`: valida FFmpeg no PATH, spawna o processo, guarda PID/estado e agenda o timer seguro. Retorna o PID.
- [x] `finalizar_corrida_manual()`: encerra a corrida ativa de forma graciosa.

## 4. Módulo `input` (hotkeys de controle)
- [ ] 4.1 Escolher backend de entrada (`gilrs` ou XInput/`windows` crate) e ler eventos em thread dedicada.
- [ ] 4.2 Persistir `HotkeyBinding` (dispositivo + botão) com fluxo de "capturar próximo botão".
- [ ] 4.3 Emitir `HotkeyEvent::Pressed/Released` independente do foco da janela.
- [ ] 4.4 Captura de microfone (`cpal`) iniciada no Pressed e encerrada no Released, em buffer na memória.
- [ ] 4.5 Enviar o áudio à Edge Function do RobotX com `room_id` e reproduzir a resposta.
- [ ] 4.6 Debounce e proteção contra hotkey presa (timeout máximo de captura).

## 5. Integração e análise
- [ ] 5.1 Comandos Tauri: `start_run`, `stop_run`, `get_run_state`.
- [ ] 5.2 Sincronização de perfis em segundo plano (spec 001, passos 6–7), com validação de SHA-256.
- [ ] 5.3 Pipeline ONNX (`ort`): amostragem de frames → regiões do perfil → inferência → `AnalysisResult`.
- [ ] 5.4 Submissão do resultado ao backend (run_id, profileVersion, variant, hash do vídeo).

## 6. Frontend (React)
- [x] 6.1a Esqueleto visual do painel de controle (tema escuro arcade, duas colunas, sem scroll global): `App.tsx`, `components/GameRoomList.tsx`, `components/RoomView.tsx`, `components/ChatFeed.tsx`, `components/ChatInput.tsx`, `types/room.ts`, `data/mockRooms.ts`. Dados e mensagens são mock local (sem WebSocket/Realtime real ainda). Botão "Iniciar Jogo via Steam" só loga no console — falta integrar com `invoke("inicializar_jogo_steam", ...)`.
- [ ] 6.1b Integrar o botão "Iniciar Jogo via Steam" ao comando Tauri real (`@tauri-apps/api` `invoke`) e aos eventos `steam://game-launched` / `steam://game-launch-timeout`.
- [ ] 6.1c HUD de run (timer) e configurações de hotkey/captura.
- [ ] 6.2 Cliente Supabase (Auth + Realtime) para salas reais (substituir mocks de `data/mockRooms.ts`).
