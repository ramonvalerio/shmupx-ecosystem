# ShmupX Offline – verificador de gameplay (sem YouTube)

Analisa um **arquivo de vídeo local** (MP4, MKV, WEBM...) de uma gameplay de shmup e gera o JSON de verificação:
placar final (hi-score), vidas perdidas, continues, game over e reinícios (attempts).

Tudo roda **no seu computador**, sem internet, usando só **Python + OpenCV + NumPy**. Não precisa de ffmpeg nem de GPU.
A API do Grok é **opcional** (`--grok`) e só é usada para conferir leituras de baixa confiança.

Jogo suportado hoje: **Earthion** (versão console com HUD em inglês SCORE/STAGE e versão PC com HUD em português PONT/FASE).
Para outros jogos, veja a seção "Como adicionar um jogo novo".

---

## 1. Instalação no Windows (passo a passo)

1. **Instale o Python 3.11 ou mais novo**
   - Baixe em https://www.python.org/downloads/ (ou rode no PowerShell: `winget install Python.Python.3.12`)
   - No instalador, marque **"Add python.exe to PATH"**.
2. Abra a pasta `shmupx_offline` deste repositório (ou uma cópia independente dela).
3. Abra o **PowerShell** nessa pasta:
   ```powershell
   cd D:\repos\ShmupX_Services\shmupx-ecosystem\shmupx_offline
   ```
4. Crie um ambiente virtual e instale as dependências:
   ```powershell
   python -m venv .venv
   .\.venv\Scripts\Activate.ps1
   pip install -r requirements.txt
   ```
   Se o PowerShell bloquear o `Activate.ps1`, rode antes:
   `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned`
5. Teste:
   ```powershell
   python analyze.py --help
   ```

**ffmpeg não é necessário.** O OpenCV já lê MP4/H.264 sozinho. (Se algum dia quiser converter vídeos,
o ffmpeg instala com `winget install Gyan.FFmpeg`, mas a ferramenta não depende dele.)

### Linux / macOS
```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
python analyze.py --help
```

---

## 2. Uso

```powershell
python analyze.py "D:\videos\earthion_1cc.mp4" --game earthion --player "iconoclast" `
    --url "https://www.youtube.com/watch?v=2LoUhboFhfA" --out resultado.json
```

Opções:

| Opção | O que faz |
|---|---|
| `VIDEO` | arquivo de vídeo local |
| `--game earthion` | script cadastrado em `shmupx/registry.py` |
| `--list-games` | lista validadores e versões |
| `--format client-v1` | envelope versionado com confiança e necessidade de revisão (padrão: `legacy`) |
| `--player NOME` | nome do jogador/canal (vai para o JSON) |
| `--url URL` | opcional, só é copiado para o JSON |
| `--game-mode 1cc` | opcional; se omitido: `1cc` sem continues, senão `Ncc` |
| `--out arquivo.json` | salva o JSON (sempre também imprime na tela) |
| `--debug-dir pasta` | salva todas as leituras (`samples.json`), o quadro final e o recorte do placar final |
| `--grok` | confere o placar final com a API do Grok **só se a confiança local for baixa** (precisa `XAI_API_KEY`) |
| `--grok-always` | manda o recorte final ao Grok mesmo com confiança alta (para auditoria) |
| `--grok-model` | padrão `grok-4.7` |
| `--brl-rate` | cotação US$→R$ para o custo estimado (padrão 5.05) |
| `--price-in`, `--price-out` | preço por 1M tokens de entrada/saída (padrão 2.00 / 6.00 US$) |
| `--quiet` | menos mensagens |

### Saída (exemplo)
```json
{
  "earthion": {
    "player": "iconoclast",
    "url": "https://www.youtube.com/watch?v=2LoUhboFhfA",
    "game-mode": "1cc",
    "duration": "00:41:58",
    "fps-analysed": 2100,
    "hi-score": 7724300,
    "continues": 0,
    "lives-lost": 0,
    "game-over": false,
    "attempts": 0,
    "observation": "O vídeo terminou antes da tela RESULTS (contagem de bônus e Total Score); o hi-score é o último placar estável do HUD, capturado em 00:38:02.",
    "total_tokens": null
  }
}
```

- `fps-analysed`: quantos quadros foram realmente analisados (amostragem + re-checagens densas).
- `total_tokens`: `null` sem `--grok`; com `--grok`, é a soma real de `usage.total_tokens` devolvida pela API
  (pode ser `0` se nenhuma leitura precisou de conferência).

### Usando o Grok (opcional, com acesso à rede)
Instale a dependência separadamente:
```powershell
python -m pip install -r requirements-grok.txt
```

```powershell
$env:XAI_API_KEY = "xai-..."     # PowerShell (só vale para esta janela)
python analyze.py video.mp4 --game earthion --player Fulano --grok
```
No Linux/macOS: `export XAI_API_KEY=xai-...`. A chave **nunca** é gravada em arquivo.
O custo estimado é impresso em US$ e R$. Uma conferência típica (1 recorte pequeno, detalhe `low`) custa frações de centavo.

---

## 3. Como funciona

1. **Calibração automática**: procura o rótulo do HUD (`SCORE` ou `PONT`) em 40 quadros espalhados pelo vídeo,
   testando escalas. Descobre a variante (console/PC), a resolução (480p, 720p, 1080p...) e o deslocamento do HUD
   (bordas, molduras). Depois refina a posição do placar, das vidas e da fase.
2. **Passada base**: lê o vídeo uma vez, analisando 1 quadro a cada 2 s (placar, vidas, fase, tela RESULTS).
   Quadros em *fade*/escurecidos ou com flash são descartados (contraste do rótulo e brilho médio).
   Durante chefes o HUD mostra `TARGET`/`ALVO` no lugar do placar; nesses quadros as vidas continuam sendo lidas.
3. **Leitura de dígitos**: recortes na resolução nativa, comparados em **tons de cinza** (sem binarizar) com
   templates de 3 fontes reais (console nítido, console com filtro CRT, PC). Cada dígito tem nota (NCC) e margem
   para o 2º colocado; leitura fraca é descartada.
   > Lição do bug 3×8 do iconoclast: nessa fonte o **3 tem traços à esquerda** e se parece com 8; só a barra do meio
   > diferencia. Por isso nada de binarizar nem de ler o placar num quadro reduzido.
4. **Validação monotônica**: o placar não pode diminuir. Leitura menor que a anterior é tratada como erro de leitura,
   a não ser que uma queda grande se confirme em 3 amostras seguidas (aí é reinício ou continue).
5. **Vidas**: toda mudança do contador `×N` (e todo trecho sem leitura > 6 s) é re-checado a 4 quadros/s, então uma
   morte seguida logo depois de vida extra não passa despercebida. Um valor só conta se aparecer em ≥3 leituras
   seguidas (ou ≥1,9 s).
6. **Placar final**:
   - Se a tela **RESULTS** aparece: lê o *Total Score*/*Pontos Totais* a 10 quadros/s e pega o último valor que ficou
     **estável em ≥10 quadros seguidos** (ignora a contagem dos bônus).
   - Se não aparece: re-lê os últimos 20 s de HUD a 6 quadros/s e usa o último valor estável em ≥10 quadros
     (maioria), e explica em `observation`.
7. **Regras**:
   - Reinício = placar volta perto de zero com fase 1 (ou fase volta para 1): `attempts` +1, zera continues/vidas
     perdidas; só a última tentativa conta.
   - Continue = vidas chegam a 0 e voltam a subir sem reinício (ou queda de placar fora da fase 1): +1 continue e
     +1 vida perdida.
   - Game over = terminou com 0 vidas e sem tela RESULTS (heurística).
   - `observation` = `null` quando não houve nada relevante.

Tempo típico: **1,5 a 3 minutos por gameplay de 40-47 min em 720p** num PC de 8 núcleos (o vídeo é decodificado uma vez).

---

## 4. Como adicionar um jogo novo

1. Crie `shmupx/validators/<jogo>.py` com regras próprias e a função `create_analyzer`. Registre ID, módulo e versão em `shmupx/registry.py`. Consulte o [contrato](../docs/specs/003-validadores-offline.md). Copiar a configuração não basta para suportar outro jogo.
2. Copie `games/earthion.json` para `games/<jogo>.json` e ajuste:
   - `hud_variants[].label_template`: recorte PNG do rótulo fixo do HUD (ex.: "SCORE"), tirado de um quadro 720p
     (ou na resolução de referência `reference_height`).
   - `alt_label_templates`: rótulos que substituem o placar em alguns momentos (ex.: chefes).
   - `score.dx/dy` (posição do 1º dígito em relação ao rótulo, em pixels da referência), `count` (nº de dígitos),
     `pitch` (distância entre dígitos); `digit_size` (largura, altura).
   - `lives.dx/dy` e `stage.dx/dy`: posição do dígito de vidas e de fase em relação ao rótulo.
   - `hud_label_search_region`, `results.label_search_region`: **frações do quadro** (0–1) onde procurar.
   - `results.label_templates`: recorte do rótulo do total na tela final (ex.: "Total Score"); remova `results` se o
     jogo não tiver tela de resultado.
3. Gere os templates de dígitos:
   ```powershell
   python tools\make_digit_templates.py video.mp4 --game <jogo> --out templates\<jogo>\digits\raw
   ```
   Olhe os `cluster_XX.png`, renomeie para `0.png`…`9.png` e mova para `templates\<jogo>\digits\<nome_do_set>\`.
   Confirme dígitos parecidos (3/8, 6/8, 1/7) usando a regra do placar crescente.
4. Rode com `--debug-dir` e confira `final_score_crop.png` e `samples.json`.

---

## 5. Limitações conhecidas

- Só Earthion está registrado. Outro jogo exige script, configuração, recursos e testes próprios (seção 4).
- Contador de vidas com **1 dígito** (0–9). Mais de 9 vidas não é lido.
- Detecção de **continue** e **game over** é heurística e não foi testada com vídeos reais com continue/game over
  (os 3 vídeos de teste são 1CC). Confira manualmente nesses casos.
- Morte + vida extra em menos de ~1 s entre si pode não ser separada (na prática não acontece no Earthion).
- O horário da morte no log é o instante em que o novo valor de vidas fica legível (pode atrasar alguns segundos se o
  HUD estiver escondido por explosões).
- Vídeos com o HUD cortado, coberto por overlays de stream (webcam, chat) ou muito comprimidos (< 360p) podem falhar
  na calibração ("HUD não encontrado").
- O placar do HUD tem 8 dígitos; o *Total Score* é lido por busca livre de dígitos à direita do rótulo.
- `--grok` envia apenas recortes do placar final quando a confiança local é baixa; o custo por imagem é estimado pela
  própria resposta (`usage`) da API.


## 6. Estrutura e manutenção

```text
analyze.py                 entrada compatível
shmupx/
  __main__.py              entrada python -m shmupx
  cli.py                   argumentos e JSON de saída
  registry.py              catálogo explícito e versões
  validators/earthion.py   algoritmo e regras do Earthion
  pipeline.py              compatibilidade com imports antigos
  video.py                 leitura compartilhada de vídeo
  hud.py                   calibração e leitura de HUD
  formatting.py            formatação comum
  grok.py                  auditoria externa opcional
games/                    configurações por ID de jogo
templates/                imagens de referência por ID de jogo
tools/                    preparação de recursos
tests/                    testes automatizados
output/                   resultados locais (ignorado pelo Git)
```

Os caminhos de configurações e templates foram preservados para manter as ferramentas existentes funcionando. Atualize a versão no registro quando as regras do jogo mudarem.

```powershell
python analyze.py --list-games
python -m shmupx --help
python -m unittest discover -s tests -v
```

O JSON ocupa stdout; progresso vai para stderr. A análise libera o vídeo também quando falha. Os testes de infraestrutura não comprovam precisão visual: ela depende de vídeos reais de referência. O cliente Tauri possui adaptador de execução ao encerrar a captura. Para configuração e limites de integração, consulte [o guia do cliente](../docs/specs/004-integracao-offline-client.md).
