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
- [x] 3.2 Linha de comando de captura real em `capture::ffmpeg::spawn_ffmpeg`: monitor principal via Desktop Duplication (`ddagrab`, GPU), reduzido **sempre para 720p** de altura (`ALTURA_GRAVACAO`, a resolução de referência dos scripts; filtro `area`), 60 fps **constantes** (`-fps_mode cfr`: o motor calcula o tempo por índice / fps), `libx264 -preset ultrafast`, sem áudio por padrão. Se o `ddagrab` não iniciar (área de trabalho remota, driver antigo), tenta `gdigrab` (todos os monitores, mais lento) com o mesmo 720p. Saída `.mp4` fragmentado (`-movflags +frag_keyframe+empty_moov`, legível mesmo após `kill` por timeout) na pasta de dados do app. Antes (2026-10-28) gravava a área de trabalho inteira na resolução nativa: com dois monitores saía 3000×1920 a ~12 fps e a análise de 6 min levava 13 min.
  - **Pendência conhecida:** o nome do dispositivo `dshow` (`DEFAULT_AUDIO_DEVICE = "virtual-audio-capturer"`) depende de um driver de loopback instalado na máquina do jogador; falta detecção/configuração em runtime (TODO em `ffmpeg.rs`, TASKS 3.1b).
- [x] 3.3 Spawn assíncrono com `tokio::process::Command`, PID armazenado em estado global (`capture::state::RunState`, `tauri::State` com `Arc<Mutex<Option<RunHandle>>>`).
- [x] 3.4 Parada graciosa (`q` no stdin + `shutdown`) com fallback para `kill` após timeout de 3s (`capture::ffmpeg::graceful_stop`, usado pelo comando `finalizar_corrida_manual`); validada contra o novo pipeline gdigrab/dshow/libx264.
- [ ] 3.5 Saída RTMP opcional (tee muxer de verdade) compartilhando o mesmo processo — `RecordingConfig.rtmp_url` hoje só adiciona uma segunda saída `-f flv`, sem `tee`.
- [ ] 3.6 Marcar a run como `invalid` no backend se o processo morrer fora do esperado (hoje só é tratado localmente).
- [ ] 3.7 Extrair metadados do arquivo final (duração, hash SHA-256) para o payload de submissão.

### Comandos Tauri expostos (capture/commands.rs)
- [x] `iniciar_corrida_safe(jogo_id)`: valida o FFmpeg (empacotado, ou do PATH em dev), spawna o processo, guarda PID/estado e agenda o timer seguro. Retorna o PID.
- [x] `finalizar_corrida_manual()`: encerra a corrida ativa de forma graciosa.

## 4. Módulo `input` (hotkeys de controle)
- [x] 4.1 Backend de entrada escolhido: `gilrs` (lê XInput/DirectInput do Windows direto, sem
      depender do foco da janela). Escuta em thread de SO dedicada (`std::thread::spawn`, não
      Tokio — a leitura do `gilrs` é bloqueante/síncrona), iniciada em `lib.rs` no hook `setup()`
      do Tauri Builder (`input::spawn_listener(app.handle().clone())`). Polling a 16ms
      (`input/gamepad.rs::POLL_INTERVAL`).
- [ ] 4.2 Persistir `HotkeyBinding` (dispositivo + botão) com fluxo de "capturar próximo botão" —
      hoje o botão é fixo (`DEFAULT_PTT_BUTTON = Button::Select`, comentado no código como TODO).
- [x] 4.3a Eventos emitidos ao frontend por transição de borda (não a cada frame com o botão
      segurado): `robotx://voice-start` no primeiro `ButtonPressed` do botão de PTT,
      `robotx://voice-end` no `ButtonReleased` correspondente — payload `{ button: string }`.
      `Gilrs::new()` falhando (sem backend de input disponível) só loga um aviso e desliga o
      push-to-talk por controle nesta sessão; não derruba o app (controle desconectado/nenhum
      controle plugado não é erro, só ausência de eventos).
- [ ] 4.3b Validar em runtime real que os eventos chegam com o jogo da Steam em foco/tela cheia e
      o Client minimizado (não testado neste ambiente — sem controle físico conectado aqui).
- [x] 4.4 Captura de microfone real (`capture/audio.rs`, via `cpal`): `iniciar_gravacao_microfone`
      (async) abre o microfone padrão do sistema e grava os samples (F32 ou I16, convertidos para
      PCM 16-bit) num `Arc<Mutex<Vec<i16>>>` compartilhado com o callback do `cpal`. Chamada pelo
      `input/gamepad.rs` no `ButtonPressed` via `tauri::async_runtime::spawn` (a thread do gamepad
      não é async). `parar_gravacao_microfone` manda o sinal de parada pelo canal `mpsc` no
      `ButtonReleased`. Estado da gravação (`RecordingState = Arc<Mutex<Option<Sender<()>>>>`)
      gerenciado via `tauri::State`, registrado em `lib.rs` (`.manage(capture::RecordingState::default())`).
      Encerramento gracioso: a própria thread de gravação dropa a `cpal::Stream` (para a captura),
      serializa o buffer como `.wav` (`hound`) em `app_data_dir()/robotx-voice/ptt_<timestamp>.wav`
      e emite `robotx://voice-ready { path }`; falhas (sem microfone, formato não suportado, erro
      de permissão/stream) emitem `robotx://voice-error { message }` em vez de panicar.
      **Nota de design:** `iniciar_gravacao_microfone`/`parar_gravacao_microfone` são funções Rust
      internas chamadas pelo listener do gamepad, não comandos Tauri invocáveis do frontend — push-
      to-talk é sempre disparado pelo controle (ADR 003), nunca por um botão de UI manual.
- [x] 4.5 Camada de rede completa (`net/backend.rs`, `net/robotx.rs`, `net/playback.rs`, novo
      bounded context `session/`):
      - `session::SessionState` guarda o JWT e o `sala_id` ativo (setados pelo React via os
        comandos `definir_token_autenticacao`/`definir_sala_ativa`) — em Rust, não só no React,
        porque os fluxos que precisam deles são autônomos (timer expirando, push-to-talk pelo
        gamepad), sem nenhum `invoke` do frontend no momento exato em que acontecem.
      - `capture::commands::iniciar_corrida_safe` agora chama `POST /api/runs/start` de
        verdade **antes** de abrir o FFmpeg (débito de ficha real) e usa o `max_duration_segundos`
        que o backend devolveu para o timer seguro — não mais o `TEST_RUN_DURATION` fixo de 5min.
      - `finalizar_corrida_manual` e o timer de expiração agora chamam `POST /api/runs` de
        verdade ao encerrar. Na época desta tarefa, `score_bruto`/`vidas_perdidas`/
        `continues_usados` eram **mockados** (sem pipeline ONNX ainda) — isso foi resolvido na
        tarefa seguinte (ver 5.3 abaixo); os valores mock continuam existindo só como fallback
        de erro, não mais como caminho principal.
      - `capture::audio.rs`: depois de salvar o `.wav` e emitir `voice-ready`, envia de verdade
        para a Edge Function `robotx-voice` (`multipart/form-data`: campo `audio` + `sala_id`,
        header `Authorization: Bearer`) e reproduz a resposta `audio/mpeg` via `rodio`
        (`net/playback.rs` — nova dependência; `cpal` só grava, não decodifica MP3). Eventos
        novos: `robotx://voice-response-ready` (upload ok, tocando) e
        `robotx://voice-response-played` (terminou de tocar).
      - Frontend (`App.tsx`): token mockado (`src/auth/token.ts::MOCK_JWT_TOKEN`, claramente
        documentado como placeholder) sincronizado com o Rust uma vez no mount; `sala_id`
        sincronizado a cada troca de sala (hoje usa `GameRoom.id`, o mock de sala do frontend —
        mesma lacuna já documentada em 6.1f, não resolvida aqui).
      - **Validado contra serviços reais, não só compilado**: subi um Postgres + o binário real
        do `shmupx-backend`, mintei um JWT HS256 de verdade e chamei `net::backend::iniciar_run`/
        `submeter_run` via um exemplo Rust temporário (removido depois) — confirmei o débito de
        ficha de verdade no banco (5→4), `max_duration_segundos=3600` vindo do backend de
        verdade (não mais o mock local), e o erro 401 tratado corretamente com token inválido.
        Para o upload multipart, subi a Edge Function `robotx-voice` real via Docker e confirmei
        que ela processa corretamente os campos `audio`/`sala_id` do meu cliente Rust (chegou até
        a etapa de resolver a sala — só falhou ali porque a URL do Supabase no teste era fake).
      - **Pendência real, não escondida**: `jogo_id` enviado ao backend hoje é o `GameRoom.id`
        mock do frontend (ex.: `"mushihimesama"`), não um UUID real de `public.jogos` — a
        chamada ao backend vai falhar (erro de desserialização de UUID) até essa lacuna (6.1f)
        ser resolvida. A camada de rede em si está correta e validada; o dado que ela carrega
        hoje ainda não é.
- [x] 4.6 Timeout máximo de 15s (`capture::audio::MAX_RECORDING_DURATION`, `pub(crate)` e
      reusado por `input/gamepad.rs`) resolve as duas pontas do bug do botão travado:
      (a) a thread de gravação corta a stream do `cpal` sozinha via `stop_rx.recv_timeout(15s)`
      mesmo sem `voice-end`; (b) o loop do gamepad destrava seu próprio `pressed = true` depois do
      mesmo intervalo, para que um `Pressed` real volte a ser aceito em vez do push-to-talk morrer
      pelo resto da sessão após uma desconexão. Ainda não testado com hardware real desconectando
      no meio de uma gravação (sem controle físico neste ambiente, ver 4.3b).

## 5. Integração e análise
- [ ] 5.1 Comandos Tauri: `start_run`, `stop_run`, `get_run_state`.
- [ ] 5.2 Sincronização de perfis em segundo plano (spec 001, passos 6–7), com validação de SHA-256.
- [x] 5.3 Pipeline ONNX (`analysis/`, novo bounded context — `crop.rs`, `frames.rs`, `model.rs`,
      `pipeline.rs`, `profile.rs`): extração eficiente de frames do final do vídeo via
      `ffmpeg -sseof` (não decodifica a gravação inteira, mesmo numa run de 60min), crop
      geométrico das regiões do perfil (`image` crate, coordenadas normalizadas → pixels
      absolutos com `clamp` defensivo), inferência local via `ort` 2.0-rc, decodificação
      softmax/argmax → dígitos → `u64`/`u8`. Resultado plugado direto em
      `capture::commands` (chamado por `finalizar_corrida_manual` e pelo timer de expiração,
      **substituindo** os valores mock como caminho principal — eles agora só entram como
      fallback se a análise falhar).
      - **Estado honesto do modelo, documentado no código (`analysis/model.rs`) e aqui**: o
        `.onnx` embutido (`models/digit_strip_placeholder.onnx`, gerado por mim via o pacote
        Python `onnx` já que não existe nenhum modelo treinado em lugar nenhum do ecossistema —
        o treinamento no Lab é um item ainda não implementado, shmupx-lab/TASKS.md seção 3) tem
        a arquitetura certa (crop → grayscale 32×128 → conv → softmax por posição de dígito),
        mas pesos **aleatórios**. Os "dígitos reconhecidos" não têm significado nenhum até o
        Lab treinar e publicar um modelo de verdade — o que está testado e funcionando é a
        mecânica do pipeline (extração → crop → tensor → inferência → decodificação →
        integração com a submissão), não a inteligência dentro do modelo.
      - **Lacuna que também apareceu**: não existe sincronização de perfis do Supabase Storage
        ainda (5.2, acima) — `analysis::profile::caminho_local` espera um `profile.json` em
        `app_data_dir()/profiles/{jogo_id}/profile.json`, que hoje só existe ali se alguém
        copiar manualmente um arquivo exportado pelo Lab. Sem ele, `analisar_run` retorna `Err`
        e `capture::commands` cai para os valores de fallback (mock) — documentado no código,
        não escondido.
      - **Validado de ponta a ponta com um vídeo e perfil reais** (não só compilado): gerei um
        vídeo sintético com `ffmpeg` (texto desenhado simulando um HUD) e um `profile.json` de
        teste, e exercitei cada etapa via um exemplo Rust temporário (removido depois) chamando
        `analysis::frames`/`crop`/`model`/`profile` diretamente: perfil carregado, 6 frames
        extraídos da janela final de 5s via `-sseof`, recorte redimensionado para 128×32,
        inferência ONNX real rodando e devolvendo 80 logits (8 posições × 10 classes), e o
        caminho de erro defensivo (vídeo inexistente) devolvendo uma mensagem clara em vez de
        panicar. 3 testes unitários cobrindo a lógica pura de softmax/argmax (classe dominante,
        logits empatados, estabilidade numérica com logits grandes — sem a subtração do máximo
        antes do `exp()`, isso daria `NaN`/`inf`).
- [ ] 5.4 Submissão do resultado ao backend (run_id, profileVersion, variant, hash do vídeo) —
      `run_id`/`jogo_id` já vão na submissão (ver 4.5); `profileVersion`/`variant`/hash do vídeo
      ainda não (o backend também não tem essas colunas em `runs` ainda).

## 6. Frontend (React)
- [x] 6.1a Esqueleto visual do painel de controle (tema escuro arcade, duas colunas, sem scroll global): `App.tsx`, `components/GameRoomList.tsx`, `components/RoomView.tsx`, `components/ChatFeed.tsx`, `components/ChatInput.tsx`, `types/room.ts`, `data/mockRooms.ts`. Dados e mensagens são mock local (sem WebSocket/Realtime real ainda).
- [x] 6.1b Botão "Iniciar Jogo via Steam" (`RoomView.tsx`) chama `invoke("inicializar_jogo_steam", { steamAppId })` via `@tauri-apps/api/core`; erro da Steam fechada é exibido inline (alerta âmbar) em vez de `console.log`.
- [x] 6.1d `App.tsx` escuta `steam://game-launched` (`@tauri-apps/api/event`) e dispara automaticamente `invoke("iniciar_corrida_safe", { jogoId })`, mapeando `app_id` → sala via `GameRoom.steamAppId`.
- [x] 6.1e `App.tsx` escuta `run://expired` e exibe modal "Tempo Expirado - Run Encerrada!"; `RoomView.tsx` desabilita o botão de iniciar e mostra indicador piscando "GRAVANDO RUN" enquanto a sala está com `isRecording = true`.
- [x] 6.1g Botão "Iniciar Jogo via Steam" agora alterna para `[ STOP ]` enquanto `isRecording`
      for `true`, chamando `invoke("finalizar_corrida_manual")` (comando Rust que já existia em
      `capture/commands.rs` mas nunca tinha sido ligado a nenhum botão da UI) — não derruba o
      processo do jogo em si (lançado via `steam://run`, fora do nosso controle), só interrompe
      a gravação/ficha ativa do lado do Client de forma graciosa. Em caso de sucesso, chama
      `onManualStop` (prop nova), que `App.tsx` usa para zerar `recordingRoomId` — o mesmo
      padrão já usado para o evento `run://expired`, só que disparado pelo clique em vez de um
      evento Tauri.
- [x] 6.1h `[ VER RANKING ONLINE ]` em `RoomView.tsx` abre `RoomRankingDrawer.tsx` (gaveta
      lateral, não modal — tema arcade, borda neon ciano): tabela com Posição, Jogador, Maior
      Placar Bruto, Vidas Perdidas, Continues (badge âmbar se `> 0`, "1CC" se `0`) e Nota
      Normalizada. **Mockado** (`data/mockRoomRanking.ts`, determinístico por `room.id`) — o
      backend não tem `GET /api/jogos/:id/scores` ainda (mesma lacuna já documentada no
      `shmupx-frontend/TASKS.md`).
- [ ] 6.1c HUD de run (timer visual com contagem regressiva) e configurações de hotkey/captura.
- [ ] 6.1f Mapeamento `app_id → sala` hoje depende do mock `GameRoom.steamAppId`; falta API para resolver isso a partir das salas reais.
- [ ] 6.1i `RoomRankingDrawer` ainda não consome `GET /api/rankings/global` nem um futuro
      endpoint por jogo — quando este existir, trocar `gerarRankingSalaMock` por uma chamada
      real (mesmo padrão de fallback com mock já usado em `shmupx-frontend`).
- [ ] 6.2 Cliente Supabase (Auth + Realtime) para salas reais (substituir mocks de `data/mockRooms.ts`).
