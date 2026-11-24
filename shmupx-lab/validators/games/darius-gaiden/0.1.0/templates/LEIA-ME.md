# Validação de Darius Gaiden (`darius-gaiden` 0.1.0)

O analisador é o genérico por HUD (`engine/analyzers/hud_score.py`, o mesmo do earthion). O que é
deste jogo é a calibração `config.json` e as imagens desta pasta `templates/`.

## Passo a passo (na pasta `shmupx-lab/validators`)

1. Recorte os rótulos do HUD de um quadro do jogo (placar, rótulo alternativo de chefe e o da tela
   de resultado) em `templates/labels/` e aponte-os no `config.json` (`hud_variants`, `results`).
2. Ajuste no `config.json` a posição do placar e das vidas (`score.dx/dy/pitch/count`, `lives`)
   em relação ao rótulo, na altura de referência de 720 px.
3. Recorte os dígitos com a ferramenta:

   ```
   python tools/make_digit_templates.py VIDEO.mp4 --game darius-gaiden --version 0.1.0
   ```

   Os grupos vão para `templates/raw_digits/`. Olhe cada `cluster_XX_nYYY.png`, renomeie para
   `0.png` … `9.png`, mova para `templates/digits/<nome_do_conjunto>/` e apague o resto.
4. Rode a análise: `python analyze.py VIDEO.mp4 --game darius-gaiden --version 0.1.0 --player teste` — ou,
   no ShmupX Lab, adicione vídeos com gabarito em **Validação** e rode esta versão até o acerto ficar
   bom. Prefira vídeos com continue ou game over para testar essas regras também.

## Estrutura de referência (do earthion)

- `digits/console_crt/0.png`
- `digits/console_crt/1.png`
- `digits/console_crt/2.png`
- `digits/console_crt/3.png`
- `digits/console_crt/4.png`
- `digits/console_crt/5.png`
- `digits/console_crt/6.png`
- `digits/console_crt/7.png`
- `digits/console_crt/8.png`
- `digits/console_crt/9.png`
- `digits/console_sharp/0.png`
- `digits/console_sharp/1.png`
- `digits/console_sharp/2.png`
- `digits/console_sharp/3.png`
- `digits/console_sharp/4.png`
- `digits/console_sharp/5.png`
- `digits/console_sharp/6.png`
- `digits/console_sharp/7.png`
- `digits/console_sharp/8.png`
- `digits/console_sharp/9.png`
- `digits/pc/0.png`
- `digits/pc/1.png`
- `digits/pc/2.png`
- `digits/pc/3.png`
- `digits/pc/4.png`
- `digits/pc/5.png`
- `digits/pc/6.png`
- `digits/pc/7.png`
- `digits/pc/8.png`
- `digits/pc/9.png`
- `labels/hud_alvo.png`
- `labels/hud_pont.png`
- `labels/hud_score.png`
- `labels/hud_target.png`
- `labels/res_total_en.png`
- `labels/res_total_pt.png`
