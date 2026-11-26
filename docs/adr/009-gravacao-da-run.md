# ADR 009 — Gravação da run: só sem memória, só a janela do jogo, com som

- **Status:** Aceito (2026-11-26)
- **Data:** 2026-11-26
- **Relaciona-se com:** a [ADR 002](002-motor-de-analise-local.md) e a [ADR 004](004-validadores-offline-por-jogo.md) (validação pelo vídeo), a [spec 002](../specs/002-economia-fichas-e-ranking.md) (timer seguro e corte da gravação).

## Contexto

A preservação da gameplay é o requisito principal do Ranked: é competitivo, e shmup não pode ter queda de quadro. A gravação roda ao lado do jogo, e até aqui o Client:

- gravava **toda** run ranked, inclusive as dos jogos validados pela memória no ShmupX Emulator, em que o vídeo não é usado (o placar vem da telemetria do motor);
- gravava o **monitor principal inteiro** (`ddagrab`, com `gdigrab` de reserva), reduzido para 720p na CPU. Pegava o que estivesse atrás ou por cima do jogo, e custava ~1,35 núcleo, dos quais ~0,9 só na redução;
- gravava **sem som**: o único caminho previsto era um dispositivo DirectShow de loopback que quase nenhum PC tem;
- em tela cheia, perdia a captura: a troca de modo de vídeo do jogo derruba o `ddagrab` (`DXGI_ERROR_ACCESS_LOST`), e a reserva `gdigrab` deixava o jogo a ~95% da velocidade, com ~13 quadros úteis por segundo no vídeo;
- abria o `ffmpeg -version` sem `CREATE_NO_WINDOW` no começo de toda run. No Client instalado (sem console), isso abria uma janela de console que tomava o foco e minimizava o jogo em tela cheia.

## Decisão

0. **Gravar é opcional e vem desligado** (atualização de 2026-11-26). Configurações › Armazenamento tem "Gravar gameplays", desmarcado por padrão. Desmarcado, nada fica gravado: os jogos validados pelo vídeo gravam só para a validação e o vídeo é apagado depois que a run é enviada (ou descartada). Marcado, toda gameplay (Ranked e Treino) é gravada e fica na pasta. A live nunca grava na pasta: ela fica no YouTube.
1. **Jogo validado pela memória não grava** (a não ser com "Gravar gameplays" marcado). O Client recebe o ROM aberto (`emulator://game-launched`) e, se ele está em `jogos-pela-memoria.json`, a run começa sem gravação. A ficha é cobrada quando o jogo abre, e a telemetria decide a run. Se a telemetria não chegar, a run falha (não há vídeo para analisar).
2. **Os outros jogos gravam só a janela do jogo**, como a live: Windows Graphics Capture da janela (não da tela), só a área interna (sem barra de título e bordas), encaixada sem distorcer em 1280×720 a 60 quadros/s. A captura segue a janela se ela mudar de tamanho ou for para tela cheia, e não grava o que estiver por cima dela.
3. **O trabalho pesado fica na placa de vídeo** (`capture/janela_nv12.rs`): um passo do processador de vídeo do Direct3D 11 recorta, reduz e converte para NV12 (BT.601, 16–235). A CPU só lê o quadro pronto, alternando entre duas texturas para não esperar a GPU. O FFmpeg codifica direto (x264 `ultrafast`), sem converter. Sem o processador de vídeo (driver antigo, área de trabalho remota), a redução cai para a CPU.
4. **Com o som do jogo:** loopback do processo do jogo (Windows 10 2004+), sem driver extra, em AAC. Sem ele, o loopback da saída padrão; sem nenhum, silêncio.
5. **O FFmpeg roda abaixo do normal e sem console.** Se faltar CPU, quem perde é o vídeo, nunca o jogo.
6. **Tela cheia gravada vai pelo bgfx.** A captura da janela vê preta a tela cheia exclusiva do Direct3D 9 (o renderizador do motor sem scanlines). Na run ranked gravada, a tela cheia sem scanlines abre pelo bgfx (com scanlines já é bgfx), que a captura vê normalmente.

## Medições (i7-9750H, 1080p, Aero Fighters 2 em janela, ~3800 quadros por rodada)

| Gravação | CPU da captura (Client) | CPU do FFmpeg | Quadros atrasados (>1,5×) |
|---|---|---|---|
| Nenhuma | — | — | 0 |
| Antiga: monitor inteiro (`ddagrab` + redução na CPU) | — | 1,35 núcleo | 0 |
| Janela, redução na CPU | 0,83 | 1,03 | 2 |
| **Janela, redução na GPU (adotada)** | **0,14** | **0,54** | **0** |

No Espgaluda II (o jogo mais pesado do catálogo), três rodadas intercaladas: 3, 5 e 4 quadros atrasados sem gravar; 2, 0 e 2 com a gravação adotada (os atrasos são do próprio jogo). Em tela cheia pelo bgfx: 0 atrasos e o jogo no vídeo.

O tempo de cada quadro foi medido dentro do motor (plugin Lua com `emu.osd_ticks()`), pelo pior caso, não pela média.

## Consequências

- (+) Nos jogos pela memória, nenhuma CPU vai para gravação durante a partida.
- (+) Nos outros, a gravação custa metade da antiga e o vídeo mostra só o jogo, com som.
- (+) A tela cheia não derruba mais a gravação (a captura é da janela, não do modo de vídeo) nem perde o foco por causa dela.
- (−) Um jogo da Steam em tela cheia exclusiva antiga (Direct3D 9) pode sair preto no vídeo; os em janela sem borda ou com as otimizações de tela cheia do Windows gravam normalmente.
- (−) Sem vídeo, a run pela memória não tem prova visual (a mesma troca da [ADR 008](008-agente-mister.md)).
- (−) O som gravado é o do jogo depois do volume do aplicativo no mixer do Windows: jogo com volume baixo no mixer grava baixo.
- (−) No Linux continua a tela inteira pelo `x11grab`, sem som (ainda sem captura de janela).
