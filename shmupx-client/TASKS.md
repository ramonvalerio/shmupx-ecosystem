# ShmupX Client — Tarefas de implementação (Rust)

Ordem sequencial. Cada item deve terminar com `cargo check` e testes unitários passando.
Contexto: `docs/adr/001..003` e `docs/specs/002`.

## 0. Fundação
- [ ] 0.1 Definir tipo de erro comum (`thiserror`) e `Result` do crate em `src-tauri/src/error.rs`.
- [ ] 0.2 Adicionar `tokio` (rt, process, time, sync) e `tracing`; inicializar runtime/estado global via `tauri::Builder::manage`.
- [ ] 0.3 Definir eventos Tauri tipados (`run://tick`, `run://expired`, `input://ptt`) e expô-los ao frontend.

## 1. Módulo `steam` (detecção de processos)
- [ ] 1.1 Adicionar `sysinfo`; implementar varredura de processos em `steam/detector.rs`.
- [ ] 1.2 Localizar a instalação da Steam (registro do Windows) e ler `libraryfolders.vdf`.
- [ ] 1.3 Ler `appmanifest_*.acf` para mapear pasta de instalação → `app_id` + nome.
- [ ] 1.4 Implementar `GameDetector` casando o executável do processo com o `installdir` do manifest.
- [ ] 1.5 Polling leve (1–2 s) emitindo `steam://game-changed` apenas em mudança.
- [ ] 1.6 Testes com fixtures de `.acf` e lista de processos fake.

## 2. Módulo `capture` — timer seguro
- [ ] 2.1 Implementar máquina de estados de `RunTimer` (Idle → Running → Expired/Finished) com `Instant`.
- [ ] 2.2 Task assíncrona de tick (1 s) emitindo `run://tick` com tempo restante.
- [ ] 2.3 Aceitar `max_duration` vindo do servidor (45–60 min) e clamp no limite superior.
- [ ] 2.4 Ao expirar, disparar sinal de corte para a sessão FFmpeg e emitir `run://expired`.
- [ ] 2.5 Testes com `tokio::time::pause` cobrindo expiração, finalização antecipada e dupla finalização.

## 3. Módulo `capture` — FFmpeg
- [ ] 3.1 Resolver o binário do FFmpeg (sidecar do Tauri ou caminho configurado) e validar versão.
- [ ] 3.2 Montar a linha de comando de captura (gdigrab/ddagrab + áudio WASAPI) a partir de `RecordingConfig`.
- [ ] 3.3 Spawn com `tokio::process`, monitorar stderr (progresso) e código de saída.
- [ ] 3.4 Parada graciosa (`q` no stdin) com fallback para kill após timeout.
- [ ] 3.5 Saída RTMP opcional (tee muxer) compartilhando o mesmo processo.
- [ ] 3.6 Integrar corte por timer (2.4) e marcar a run como `invalid` se o processo morrer fora do esperado.
- [ ] 3.7 Extrair metadados do arquivo final (duração, hash SHA-256) para o payload de submissão.

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
- [ ] 6.1 Telas: lista de salas, sala, HUD de run (timer), configurações de hotkey/captura.
- [ ] 6.2 Cliente Supabase (Auth + Realtime) para salas.
