# Scripts de validação do ShmupX Lab

Analisam um **arquivo de vídeo local** de uma gameplay de shmup e geram o JSON de verificação: placar final
(hi-score), vidas perdidas, continues, game over e reinícios (attempts). Tudo roda no computador, só com
**Python + OpenCV + NumPy** (sem ffmpeg, sem GPU). O Grok é opcional (`--grok`) e só confere leituras de
baixa confiança.

O Lab usa esta pasta na tela **Validação**: cada jogo tem seus scripts, e cada script é uma versão.

## Estrutura

```text
validators/
  analyze.py                     entrada da linha de comando
  engine/                        motor genérico (igual para todos os jogos)
    cli.py                       argumentos e JSON de saída (legacy e client-v1)
    catalog.py                   descobre os scripts em games/<jogo>/<versão>/
    hud.py                       calibração do HUD e leitura de dígitos
    video.py                     leitura do vídeo (OpenCV)
    report.py                    contrato client-v1 (o mesmo que o Client lê)
    grok.py                      auditoria externa opcional
    analyzers/hud_score.py       analisador por HUD: placar, vidas, continues, attempts
  games/
    <jogo>/
      <versão>/                  um script = uma pasta, ex.: games/earthion/1.0.0/
        script.json              jogo, nome, versão, status (draft|approved), analisador
        config.json              calibração do HUD deste jogo
        templates/               rótulos (labels/) e dígitos (digits/<conjunto>/0-9.png)
  tools/make_digit_templates.py  recorta os dígitos de um vídeo para um script novo
  tests/                         testes do motor e do catálogo
```

A lógica de leitura é genérica. O que é de cada jogo é a **calibração** e as **imagens**, e elas ficam na
pasta da versão: mudar a calibração ou trocar um dígito é criar uma versão nova, e a anterior continua
reproduzível. O `script.json` escolhe o analisador **por nome** (`hud_score`); IDs e versões são conferidos
por expressão regular e nunca viram caminhos ou imports livres.

Sem `--version`, vale a versão **mais nova aprovada** (ou a mais nova, se o jogo só tiver rascunhos).
Versões novas sempre nascem `draft`.

## Instalação (Windows)

```powershell
cd shmupx-lab\validators
py -3.12 -m venv .venv
.\.venv\Scripts\pip install -r requirements.txt
python analyze.py --list-games
```

O Lab usa o Python de `SHMUPX_PYTHON`, senão o `.venv` desta pasta, senão o do PATH. Fora do modo de
desenvolvimento, aponte `SHMUPX_VALIDATORS_ROOT` para esta pasta.

## Uso

```powershell
python analyze.py "D:\videos\earthion_1cc.mp4" --game earthion --player "iconoclast" --out resultado.json
python analyze.py video.mp4 --game earthion --version 1.1.0 --player teste --format client-v1
```

| Opção | O que faz |
|---|---|
| `--game ID` | jogo (pasta `games/<ID>`) |
| `--version X.Y.Z` | versão do script (padrão: a mais nova aprovada) |
| `--list-games` | lista os scripts: jogo, nome, versão e status |
| `--format client-v1` | envelope versionado com confiança e necessidade de revisão (padrão: `legacy`) |
| `--player NOME` | nome do jogador/canal |
| `--url URL`, `--game-mode 1cc` | opcionais, só vão para o JSON |
| `--out arquivo.json` | também salva o JSON |
| `--debug-dir pasta` | salva as leituras (`samples.json`), o quadro final e o recorte do placar |
| `--progress` | publica `SHMUPX_PROGRESS <0-100>` no stderr (o Lab usa) |
| `--grok`, `--grok-always` | conferência pelo Grok (precisa `XAI_API_KEY`; `pip install -r requirements-grok.txt`) |
| `--quiet` | menos mensagens |

O JSON ocupa o stdout; logs e progresso vão para o stderr. O `validator_version` do relatório é a versão
do script que rodou.

## Script para um jogo novo

No Lab, **Validação → Gerar script** cria `games/<jogo>/0.1.0/` (rascunho) com a calibração do Earthion
como ponto de partida, as pastas das imagens e um `templates/LEIA-ME.md` com o passo a passo:

1. Recorte os rótulos do HUD em `templates/labels/` e aponte-os no `config.json` (`hud_variants`, `results`).
2. Ajuste a posição do placar e das vidas (`score.dx/dy/pitch/count`, `lives`) na referência de 720 px.
3. Recorte os dígitos:
   ```powershell
   python tools\make_digit_templates.py video.mp4 --game <jogo> --version 0.1.0
   ```
   Os grupos vão para `templates/raw_digits/`. Renomeie para `0.png`…`9.png`, mova para
   `templates/digits/<conjunto>/` e apague o resto. Confirme 3/8 e 6/8 pela regra do placar crescente.
4. Adicione vídeos com gabarito no Lab e rode a versão até o acerto ficar bom.

Para melhorar um script existente, **Nova versão** copia a versão escolhida para a próxima (ex.: 1.0.0 →
1.1.0, rascunho), sem os recortes brutos. A versão de origem fica intacta.

## Como o analisador por HUD funciona

1. **Calibração**: procura o rótulo do HUD em 40 quadros, testando escalas; descobre variante, resolução e
   deslocamento, e refina a posição do placar, das vidas e da fase.
2. **Passada base**: 1 quadro a cada `sample_interval_s`; descarta quadros em fade ou com flash.
3. **Dígitos**: comparação em tons de cinza (sem binarizar) com os conjuntos de `templates/digits/`, com
   nota e margem para o 2º colocado.
4. **Placar monotônico**: leitura menor que a anterior é erro de leitura, salvo queda grande confirmada
   (reinício ou continue).
5. **Vidas**: toda mudança é re-checada em alta taxa; um valor só conta se aparecer em leituras seguidas.
6. **Placar final**: total da tela de resultado estável em ≥10 quadros, senão o último placar estável do HUD.
7. **Regras**: reinício (attempts), continue, game over heurístico e `observation` explicando os casos.

## Limitações conhecidas

- Contador de vidas com 1 dígito.
- Continue e game over são heurísticos e ainda não foram validados com vídeos reais desses casos.
- HUD cortado, coberto por overlays ou vídeo muito comprimido (< 360p) pode falhar na calibração.

## Executável para o Client

O Client instalado não usa Python: roda este motor compilado com PyInstaller.

```powershell
.	oolsuild_validator.ps1     # gera dist\shmupx-validator\shmupx-validator.exe
dist\shmupx-validator\shmupx-validator.exe --list-games --games-dir games
```

O build usa um ambiente isolado (`.venv-build`) com as versões fixas de `requirements-build.txt`. Os scripts dos
jogos (`games/`) não entram no binário: quem chama passa `--games-dir` (ou `SHMUPX_GAMES_DIR`); sem isso, o
`.exe` procura `games/` ao lado dele. O `--grok` fica de fora do executável. Mudar o motor ou as versões fixas é
mudar o que os jogadores rodam: compare os vídeos de referência no Lab antes de lançar.

## Testes

```powershell
python -m unittest discover -s tests -v
```

Os testes cobrem a infraestrutura (catálogo, CLI, contrato); a precisão visual é medida no Lab, com
vídeos reais e gabarito.
