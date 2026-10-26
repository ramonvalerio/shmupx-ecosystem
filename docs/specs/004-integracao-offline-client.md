# Execução offline pelo cliente

## Implementado

`capture/commands.rs` encerra o FFmpeg, emite `run://analysis` e chama `analysis/offline.rs`. O adaptador executa o script Python por argumentos separados, sem shell e sem abrir janela no Windows. Não envia vídeo nem usa Grok.

O processo tem limite de 15 minutos, stdout limitado a 1 MiB e stderr a 64 KiB. Os pipes são drenados em paralelo; excesso de saída, erro de processo, JSON inválido, versão incompatível e métricas inválidas impedem a submissão. Timeout encerra o processo.

O contrato aceito é `client-v1`, Earthion `1.0.0`. `needs_review`, confiança diferente de `high`, placar ausente, game over e continues impedem envio automático. `confidence: high` é a classificação do script, não garantia antifraude. Ao mudar a versão do validador, atualize deliberadamente a compatibilidade do cliente e seus testes.

Somente a confirmação HTTP do backend gera o estado `submitted`. Falhas geram `failed` com motivo; nenhum placar substituto é enviado. O vídeo fica na pasta de dados do app, em `shmupx_run_<run-id>.mp4`. O histórico exibido pela interface ainda é apenas da sessão.

A gravação e o processamento são mutuamente exclusivos. O backend atual escolhe a última run ativa por jogador/jogo, sem receber `run_id` na submissão; liberar outra captura antes do envio poderia associar os pontos à run errada. Timers usam o ID da run, evitando cortar uma partida posterior do mesmo jogo.

## Ambiente de desenvolvimento

Na raiz `shmupx_offline`, prepare o Python e as dependências:

```powershell
python -m venv .venv
.\.venv\Scripts\python.exe -m pip install -r requirements.txt
.\.venv\Scripts\python.exe analyze.py --list-games
```

O build de desenvolvimento encontra automaticamente o diretório irmão `shmupx_offline` e seu `.venv`. Sem esse ambiente, tenta `python` no PATH. É possível definir caminhos explícitos antes de iniciar o cliente:

```powershell
$env:SHMUPX_OFFLINE_ROOT = "D:\repos\ShmupX_Services\shmupx-ecosystem\shmupx_offline"
$env:SHMUPX_PYTHON = "$env:SHMUPX_OFFLINE_ROOT\.venv\Scripts\python.exe"
```

`SHMUPX_PYTHON` deve conter o caminho do executável, sem opções ou aspas embutidas. Essas variáveis são configuração local do processo, não valores recebidos por um formulário remoto.

## Pendências para uso real

- O catálogo da interface ainda contém jogos de demonstração. O backend exige UUID; o script usa `earthion`. É necessário carregar o catálogo real e associar seu UUID ao script. Não inventar um UUID nem reutilizar um jogo diferente.
- Autenticação, ficha real e captura dependem das integrações existentes; não foram exercitadas nesta etapa.
- Instalação de produção: o adaptador procura `shmupx_offline` na pasta de recursos, ou no caminho explícito acima. O instalador ainda não inclui scripts/runtime Python/OpenCV. Não considerar o pacote desktop autossuficiente.
- Revisão/reenvio de falhas e encerramento da ficha no backend após falha de análise ainda precisam de fluxo próprio. Não reabrir uma run somente para reenviar o vídeo.
- Validar precisão e desempenho com vídeos reais, incluindo reinício, continue, game over, HUD ausente e resultados de baixa confiança.

## Verificação local

```powershell
# Em shmupx_offline
python -m unittest discover -s tests -v

# Em shmupx-client
npm run build
npm run test:ui
cd src-tauri
cargo test --offline --lib analysis::offline
```

Os testes cobrem contrato, rejeição de resultados e estados da interface; não substituem uma sessão real de captura e submissão.
