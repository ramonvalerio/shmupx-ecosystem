# Execução offline pelo cliente

## Implementado

`capture/commands.rs` encerra o FFmpeg, emite `run://analysis` e chama `analysis/offline.rs`. O adaptador executa o motor de validação por argumentos separados, sem shell e sem abrir janela no Windows, passando `--games-dir` com a pasta dos scripts. Não envia vídeo nem usa Grok.

O processo tem limite de 15 minutos, stdout limitado a 1 MiB e stderr a 64 KiB. Os pipes são drenados em paralelo; excesso de saída, erro de processo, JSON inválido, versão incompatível e métricas inválidas impedem a submissão. Timeout encerra o processo.

O contrato aceito é `client-v1`. Os scripts aceitos (jogo → versão) estão em `src-tauri/scripts-aceitos.json`, hoje Earthion `1.0.0`. `needs_review`, confiança diferente de `high`, placar ausente, game over e continues impedem envio automático. `confidence: high` é a classificação do script, não garantia antifraude. Ao mudar a versão do validador, atualize deliberadamente a compatibilidade do cliente e seus testes.

Somente a confirmação HTTP do backend gera o estado `submitted`. Falhas geram `failed` com motivo; nenhum placar substituto é enviado. O vídeo fica na pasta de dados do app, em `shmupx_run_<run-id>.mp4`. O histórico exibido pela interface ainda é apenas da sessão.

A gravação e o processamento são mutuamente exclusivos. A submissão envia o `run_id` devolvido no início, e o backend grava o resultado nessa gameplay do Firestore. Timers usam o ID da run, evitando cortar uma partida posterior do mesmo jogo.

## Motor e FFmpeg

| Modo | Motor | Scripts dos jogos | FFmpeg |
| --- | --- | --- | --- |
| Instalado | `validator/shmupx-validator.exe` nos recursos | `games/` nos recursos | `ffmpeg/ffmpeg.exe` nos recursos |
| Desenvolvimento | `python analyze.py` de `shmupx-lab/validators` | `shmupx-lab/validators/games` | o do PATH |

O jogador não instala Python nem FFmpeg. O `.exe` é o mesmo motor Python compilado com PyInstaller (`shmupx-lab/validators/tools/build_validator.ps1`, versões fixas em `requirements-build.txt`); os scripts dos jogos ficam fora dele, como dados. Em desenvolvimento, o adaptador usa `shmupx-lab/validators` e o `.venv` dessa pasta, senão `python` do PATH. Variáveis locais do processo:

- `SHMUPX_VALIDATORS_ROOT`: outra pasta `validators` (com `analyze.py` e `games/`);
- `SHMUPX_PYTHON`: caminho do executável Python, sem opções ou aspas embutidas;
- `SHMUPX_VALIDATOR_EXE`: usa o `.exe` compilado também em desenvolvimento, para testá-lo.

## Instalador

```powershell
# em shmupx-client: compila o motor, junta motor + scripts aceitos + FFmpeg em src-tauriundle\ e gera o instalador
npm run bundle
# reaproveitando o motor já compilado:
powershell -File scripts\prepare-bundle.ps1 -ReusarMotor
npm run tauri build -- --config src-tauri/tauri.bundle.conf.json --config src-tauri/tauri.bundle.windows.conf.json
```

O `prepare-bundle.ps1` só empacota scripts aprovados e confere que o motor empacotado os enxerga completos. O `tauri dev` não usa `tauri.bundle.conf.json`. O FFmpeg nunca vem do PC: o `scripts/build-ffmpeg.sh` compila a última versão do git.ffmpeg.org (com x264 do code.videolan.org) pelo MSYS2 e reaproveita o build quando não há versão nova. É GPL (libx264): a licença vai junto em `ffmpeg/LICENSE.txt`, e a distribuição precisa cumprir a GPL do FFmpeg.

O instalador do Windows também leva, ao lado do `shmupx-client.exe`, as DLLs do Visual C++ (`msvcp140*`, `vcruntime140*`, exigidas pelo ONNX Runtime) e o `DirectML.dll` 1.15 (`src-tauriundleuntime\`, via `tauri.bundle.windows.conf.json`), e embute o instalador do WebView2. Assim o Client abre num Windows recém-instalado, sem o jogador instalar o Visual C++ Redistributable.

### ShmupX Portable

O `npm run bundle` também gera `targeteleaseundle\portable\ShmupX-Portable-<versão>.exe` (`scripts\make-portable.ps1`): o lançador `shmupx-client\portable` (compilado com `+crt-static`, sem depender do Visual C++) com a pasta do Client inteira anexada num `.zip`. Dois cliques: na primeira vez de cada versão ele descompacta em `%LOCALAPPDATA%\ShmupX Portablepp\<versão>\` (instala o WebView2 pelo instalador oficial se faltar) e abre o Client; nas seguintes abre direto. Um `.exe` antigo abre a versão mais nova já descompactada, nunca volta de versão.

Com a marca `.portable` ao lado do exe, o Client guarda `roms` e `recordings` em `ShmupX Portable\`, fora da pasta da versão, e a atualização automática baixa o próximo ShmupX Portable (SHA-256 conferido com o backend) em vez do instalador NSIS. O Lab envia os dois juntos (`PUT /api/admin/releases/{versão}/installer` e `.../portable`); o site oferece o portátil pelo `portable` de `GET /api/client/downloads`.

## Pendências para uso real

- O catálogo da interface ainda contém jogos de demonstração. O backend usa `games/{gameId}` do Firestore com o mesmo ID do script (`earthion`); o documento ainda precisa ser publicado. Não reutilizar um jogo diferente.
- Autenticação, ficha real e captura dependem das integrações existentes; não foram exercitadas nesta etapa.
- Atualização de scripts sem nova versão do Client (pacotes assinados publicados pelo Lab) e atualizador do Tauri ainda não existem: motor e scripts mudam com o instalador.
- Revisão/reenvio de falhas e encerramento da ficha no backend após falha de análise ainda precisam de fluxo próprio. Não reabrir uma run somente para reenviar o vídeo.
- Validar precisão e desempenho com vídeos reais, incluindo reinício, continue, game over, HUD ausente e resultados de baixa confiança.

## Verificação local

```powershell
# Em shmupx-lab/validators
python -m unittest discover -s tests -v

# Em shmupx-client
npm run build
npm run test:ui
cd src-tauri
cargo test --offline --lib analysis::offline
```

Os testes cobrem contrato, rejeição de resultados e estados da interface; não substituem uma sessão real de captura e submissão.
