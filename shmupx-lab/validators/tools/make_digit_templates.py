#!/usr/bin/env python3
"""Ajuda a criar templates de dígitos para um jogo novo.

Uso:
  python tools/make_digit_templates.py VIDEO.mp4 --game NOVOJOGO --version 0.1.0

Requer games/NOVOJOGO/<versão>/config.json com o rótulo do HUD (label_template) e a posição do placar
(score.dx/dy/pitch/count) já ajustados. Os recortes vão para templates/raw_digits da versão (ou --out),
fora de templates/digits para não serem lidos como um conjunto de dígitos. O script lê as células do placar a cada N segundos, agrupa recortes parecidos
(em tons de cinza, sem binarizar) e salva a mediana de cada grupo como cluster_XX_nYYY.png.
Depois, OLHE cada cluster e renomeie para 0.png ... 9.png (apague o que não for dígito).
Dica: confirme 3 x 8 e 6 x 8 pela regra do placar crescente (o placar nunca diminui).
"""
import argparse
import os
import sys

import cv2
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from engine.catalog import get_script  # noqa: E402
from engine.hud import GameConfig, Hud  # noqa: E402
from engine.video import Video  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("video")
    ap.add_argument("--game", required=True)
    ap.add_argument("--version", help="versão do script (padrão: a mais nova aprovada)")
    ap.add_argument("--out", help="pasta dos recortes (padrão: templates/raw_digits da versão)")
    ap.add_argument("--interval", type=float, default=10.0)
    ap.add_argument("--max-dist", type=float, default=12.0, help="distância média (0-255) para juntar recortes")
    a = ap.parse_args()
    try:
        script = get_script(a.game, a.version)
    except ValueError as exc:
        ap.error(str(exc))
    gc = GameConfig(script.path)
    a.out = a.out or os.path.join(script.path, "templates", "raw_digits")
    v = Video(a.video)
    hud = Hud(gc)
    frames = [cv2.cvtColor(f, cv2.COLOR_BGR2GRAY).astype(np.float32)
              for f in (v.frame_at(v.duration * k / 41) for k in range(1, 41)) if f is not None]
    if not hud.calibrate(frames, v.height):
        sys.exit("Rótulo do HUD não encontrado; ajuste label_template no JSON do jogo.")
    c = gc["score"]
    dw, dh = gc["digit_size"]
    dx, dy = hud.off("score")
    clusters = []  # [soma, n]
    for t, f in v.sequential(a.interval):
        g = hud.to_ref(f)
        if hud.label_visible(g) < gc["hud_label_min_score"]:
            continue
        lx, ly = hud.label_pos
        for k in range(c["count"]):
            x, y = int(round(lx + dx + c["pitch"] * k)), int(round(ly + dy))
            cell = g[y:y + dh, x:x + dw]
            if cell.shape != (dh, dw):
                continue
            for cl in clusters:
                if np.abs(cl[0] / cl[1] - cell).mean() < a.max_dist:
                    cl[0] += cell
                    cl[1] += 1
                    break
            else:
                clusters.append([cell.copy(), 1])
    os.makedirs(a.out, exist_ok=True)
    clusters.sort(key=lambda c: -c[1])
    for i, (s, n) in enumerate(clusters[:30]):
        cv2.imwrite(os.path.join(a.out, f"cluster_{i:02d}_n{n}.png"), np.clip(s / n, 0, 255).astype(np.uint8))
    print(f"{len(clusters)} grupos; os 30 maiores foram salvos em {a.out}. Renomeie para 0.png..9.png.")


if __name__ == "__main__":
    main()
