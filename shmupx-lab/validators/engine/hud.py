"""Calibração do HUD e leitura de dígitos por template matching em tons de cinza."""
import json
import os

import cv2
import numpy as np


def _imread_gray(path):
    data = np.fromfile(path, dtype=np.uint8)  # funciona com caminhos Unicode no Windows
    img = cv2.imdecode(data, cv2.IMREAD_GRAYSCALE)
    if img is None:
        raise SystemExit(f"Template não encontrado: {path}")
    return img.astype(np.float32)


class GameConfig:
    """Calibração e imagens de um script: <pasta da versão>/config.json e <pasta da versão>/templates/."""

    def __init__(self, script_dir):
        cfg_path = os.path.join(script_dir, "config.json")
        if not os.path.exists(cfg_path):
            raise SystemExit(f"Script sem calibração (falta {cfg_path})")
        with open(cfg_path, encoding="utf-8") as f:
            self.cfg = json.load(f)
        self.tdir = os.path.join(script_dir, "templates")
        # conjuntos de dígitos: templates/digits/<conjunto>/<0-9>.png
        ddir = os.path.join(self.tdir, self.cfg.get("digit_templates", "digits"))
        self.digit_sets = {}
        for name in sorted(os.listdir(ddir)):
            p = os.path.join(ddir, name)
            if os.path.isdir(p):
                self.digit_sets[name] = {
                    d: _imread_gray(os.path.join(p, f"{d}.png")) for d in "0123456789"
                    if os.path.exists(os.path.join(p, f"{d}.png"))
                }
        self.variants = [
            dict(v, template=_imread_gray(os.path.join(self.tdir, v["label_template"])),
                 alt_templates=[_imread_gray(os.path.join(self.tdir, p)) for p in v.get("alt_label_templates", [])])
            for v in self.cfg["hud_variants"]
        ]
        r = self.cfg.get("results")
        self.results_labels = [_imread_gray(os.path.join(self.tdir, p)) for p in r["label_templates"]] if r else []

    def __getitem__(self, k):
        return self.cfg[k]

    def get(self, k, default=None):
        return self.cfg.get(k, default)


def _region(shape, frac):
    h, w = shape[:2]
    return (int(frac["x0"] * w), int(frac["y0"] * h), int(frac["x1"] * w), int(frac["y1"] * h))


def _best_match(img, tmpl):
    if img.shape[0] < tmpl.shape[0] or img.shape[1] < tmpl.shape[1]:
        return -1.0, (0, 0)
    m = cv2.matchTemplate(img, tmpl, cv2.TM_CCOEFF_NORMED)
    _, mx, _, loc = cv2.minMaxLoc(m)
    return float(mx), loc


class Hud:
    """Guarda escala/variante/posição do HUD e lê dígitos em coordenadas de referência (720p)."""

    def __init__(self, gc):
        self.gc = gc
        self.scale = None      # pixels do vídeo por pixel de referência
        self.variant = None
        self.label_pos = None  # (x, y) do rótulo SCORE/PONT em coordenadas de referência
        self.label_score = 0.0

    # ---------- calibração ----------
    def calibrate(self, gray_frames, video_h):
        cfg = self.gc
        ref_h = cfg["reference_height"]
        s0 = video_h / ref_h
        ss = cfg["scale_search"]
        scan_frames = gray_frames
        coarse = list(np.arange(s0 * 0.85, s0 * 1.15 + 1e-9, 0.01 * s0))
        best = self._scan(scan_frames, coarse)
        if best[0] < cfg["hud_label_min_score"]:
            coarse = list(np.arange(ss["min"], ss["max"] + 1e-9, ss["coarse_step"]))
            best = self._scan(scan_frames, coarse)
        if best[0] < cfg["hud_label_min_score"]:
            return False
        s = best[1]
        fine = list(np.arange(s - 0.025 * s0, s + 0.025 * s0 + 1e-9, ss["fine_step"] * s0))
        best = self._scan(gray_frames, fine, variants=[best[2]])
        score, s, vi, positions = best
        self.scale = float(s)
        self.variant = self.gc.variants[vi]
        xs = [p[0] for p in positions]
        ys = [p[1] for p in positions]
        self.label_pos = (float(np.median(xs)) / s, float(np.median(ys)) / s)
        self.label_score = score
        # contraste típico do rótulo (para descartar quadros em fade/escurecidos)
        stds = []
        th, tw = self.variant["template"].shape
        for g in gray_frames:
            gr = cv2.resize(g, None, fx=1 / s, fy=1 / s, interpolation=cv2.INTER_AREA) if abs(s - 1) > 0.01 else g
            if self.label_visible(gr, check_contrast=False) >= self.gc["hud_label_min_score"]:
                x, y = int(round(self.label_pos[0])), int(round(self.label_pos[1]))
                stds.append(float(gr[y:y + th, x:x + tw].std()))
        self.label_std = float(np.median(stds)) if stds else 0.0
        # refina a posição de cada elemento (pequenos erros de escala viram deslocamentos longe do rótulo)
        self.offsets = {}
        for key in ("score", "lives", "stage"):
            c = self.gc.get(key)
            if not c:
                continue
            found = []
            for g in gray_frames:
                gr = cv2.resize(g, None, fx=1 / s, fy=1 / s, interpolation=cv2.INTER_AREA) if abs(s - 1) > 0.01 else g
                if self.label_visible(gr, check_contrast=False) < self.gc["hud_label_min_score"]:
                    continue
                # placar: janela pequena (< meio dígito) para não "pular" para a célula vizinha
                pad = 10 if key == "score" else 22
                d, sc, mg, (fx, fy) = self.read_digit(gr, self.label_pos[0] + c["dx"], self.label_pos[1] + c["dy"], pad)
                if sc >= 0.8:
                    found.append((fx - self.label_pos[0], fy - self.label_pos[1]))
            if len(found) >= 3:
                self.offsets[key] = (float(np.median([f[0] for f in found])), float(np.median([f[1] for f in found])))
            else:
                self.offsets[key] = (c["dx"], c["dy"])
        return True

    def off(self, key):
        c = self.gc[key]
        return getattr(self, "offsets", {}).get(key, (c["dx"], c["dy"]))

    def _scan(self, frames, scales, variants=None):
        """Para cada escala candidata, reduz/amplia a região de busca do quadro para a
        resolução de referência e compara com o template original (mais fiel que escalar o template)."""
        best = (-1, 1.0, 0, [])
        vis = variants if variants is not None else range(len(self.gc.variants))
        regions = []
        for g in frames:
            x0, y0, x1, y1 = _region(g.shape, self.gc["hud_label_search_region"])
            regions.append((g[y0:y1, x0:x1], x0, y0))
        for s in scales:
            scaled = [(cv2.resize(r, None, fx=1 / s, fy=1 / s, interpolation=cv2.INTER_AREA if s > 1 else cv2.INTER_CUBIC), x0, y0)
                      for r, x0, y0 in regions]
            for vi in vis:
                t = self.gc.variants[vi]["template"]
                th, tw = t.shape
                scores, pos = [], []
                for r, x0, y0 in scaled:
                    sc, loc = _best_match(r, t)
                    if sc > 1.0001 or r[loc[1]:loc[1] + th, loc[0]:loc[0] + tw].std() < 10:
                        continue  # janela sem textura (NCC degenerado)
                    if sc > 0.6:
                        scores.append(sc)
                        pos.append((loc[0] * s + x0, loc[1] * s + y0))
                if len(scores) >= 2:
                    q = float(np.mean(sorted(scores)[-max(2, len(scores) // 2):]))
                    if q > best[0]:
                        best = (q, s, vi, pos)
        return best

    # ---------- conversão ----------
    def to_ref(self, frame_bgr):
        g = cv2.cvtColor(frame_bgr, cv2.COLOR_BGR2GRAY)
        if abs(self.scale - 1.0) > 0.01:
            g = cv2.resize(g, None, fx=1.0 / self.scale, fy=1.0 / self.scale, interpolation=cv2.INTER_AREA if self.scale > 1 else cv2.INTER_CUBIC)
        return g.astype(np.float32)

    # ---------- leitura ----------
    def alt_label_visible(self, g):
        """Rótulo alternativo (ex.: TARGET/ALVO durante chefes): placar oculto, vidas visíveis."""
        best = 0.0
        for t in self.variant.get("alt_templates", []):
            lx, ly = self.label_pos
            x, y, p = int(round(lx)), int(round(ly)), 6
            sc, _ = _best_match(g[max(0, y - p):y + t.shape[0] + p, max(0, x - p):x + t.shape[1] + p], t)
            best = max(best, sc)
        return best

    def label_visible(self, g, check_contrast=True):
        lx, ly = self.label_pos
        t = self.variant["template"]
        p = 6
        x, y = int(round(lx)), int(round(ly))
        sc, _ = _best_match(g[max(0, y - p):y + t.shape[0] + p, max(0, x - p):x + t.shape[1] + p], t)
        if check_contrast and getattr(self, "label_std", 0) > 0:
            if g[y:y + t.shape[0], x:x + t.shape[1]].std() < 0.8 * self.label_std:
                return 0.0  # HUD em fade/escurecido: não confiável
        return sc

    def read_digit(self, g, x, y, pad, scale=1.0):
        """Lê um dígito numa janela (x,y,pad). Devolve (dígito, ncc, margem, (x,y))."""
        dw, dh = self.gc["digit_size"]
        x, y = int(round(x)), int(round(y))
        reg = g[max(0, y - pad):y + int(dh * scale) + pad, max(0, x - pad):x + int(dw * scale) + pad]
        per_digit = {}
        for tset in self.gc.digit_sets.values():
            for d, t in tset.items():
                if scale != 1.0:
                    t = cv2.resize(t, None, fx=scale, fy=scale, interpolation=cv2.INTER_AREA)
                sc, loc = _best_match(reg, t)
                if sc > per_digit.get(d, (-2,))[0]:
                    per_digit[d] = (sc, loc)
        if len(per_digit) < 2:
            # Script novo, ainda sem dígitos recortados: nenhuma leitura é confiável.
            return "0", -1.0, 0.0, (x, y)
        ranked = sorted(per_digit.items(), key=lambda kv: kv[1][0], reverse=True)
        (d1, (s1, loc)), (_, (s2, _)) = ranked[0], ranked[1]
        return d1, s1, s1 - s2, (max(0, x - pad) + loc[0], max(0, y - pad) + loc[1])

    def read_score(self, g):
        c = self.gc["score"]
        lx, ly = self.label_pos
        dx, dy = self.off("score")
        x0, y0 = lx + dx, ly + dy
        s, mins, minm = "", 1.0, 1.0
        prev_x = None
        for k in range(c["count"]):
            x = x0 + c["pitch"] * k if prev_x is None else prev_x + c["pitch"]
            d, sc, mg, (fx, fy) = self.read_digit(g, x, y0, c["pad"])
            prev_x = fx if abs(fx - x) <= c["pad"] else x
            s += d
            mins, minm = min(mins, sc), min(minm, mg)
        ok = mins >= self.gc["digit_min_score"] and minm >= self.gc["digit_min_margin"]
        return s, ok, mins, minm

    def read_small(self, g, key):
        c = self.gc[key]
        lx, ly = self.label_pos
        dx, dy = self.off(key)
        d, sc, mg, _ = self.read_digit(g, lx + dx, ly + dy, c["pad"])
        ok = sc >= self.gc["digit_min_score"] and mg >= self.gc["digit_min_margin"]
        return int(d), ok, sc

    def score_crop(self, frame_bgr, margin=6):
        """Recorte do placar na resolução NATIVA do vídeo (para debug/Grok)."""
        c = self.gc["score"]
        dw, dh = self.gc["digit_size"]
        lx, ly = self.label_pos
        s = self.scale
        dx, dy = self.off("score")
        x0 = int((lx - margin) * s)
        y0 = int((ly + dy - margin) * s)
        x1 = int((lx + dx + c["pitch"] * c["count"] + margin) * s)
        y1 = int((ly + dy + dh + margin) * s)
        return frame_bgr[max(0, y0):y1, max(0, x0):x1]

    # ---------- tela de resultados ----------
    def find_results_label(self, g):
        r = self.gc["results"]
        x0, y0, x1, y1 = _region(g.shape, r["label_search_region"])
        best = (-1, None, None)
        for t in self.gc.results_labels:
            sc, loc = _best_match(g[y0:y1, x0:x1], t)
            if sc > best[0]:
                best = (sc, (loc[0] + x0, loc[1] + y0), t.shape)
        return best

    def read_results_total(self, g, label):
        """Lê o número à direita do rótulo Total Score com busca livre de dígitos."""
        r = self.gc["results"]
        sc, (lx, ly), (th, tw) = label
        b = r["digits_band"]
        x0 = lx + tw + b["dx0"]
        band = g[max(0, ly + b["dy0"]):ly + b["dy1"], x0:x0 + b["dx1"]]
        dw, dh = self.gc["digit_size"]
        cands = []
        for scale in r["digit_scales"]:
            for tset in self.gc.digit_sets.values():
                for d, t in tset.items():
                    tt = cv2.resize(t, None, fx=scale, fy=scale, interpolation=cv2.INTER_AREA) if scale != 1 else t
                    if band.shape[0] < tt.shape[0] or band.shape[1] < tt.shape[1]:
                        continue
                    m = cv2.matchTemplate(band, tt, cv2.TM_CCOEFF_NORMED)
                    ys, xs = np.where(m > 0.72)
                    for yy, xx in zip(ys, xs):
                        cands.append((float(m[yy, xx]), int(xx), int(yy), d, scale))
        if not cands:
            return None, 0.0
        cands.sort(reverse=True)
        row_y = cands[0][2]
        cands = [c for c in cands if abs(c[2] - row_y) <= 5]
        chosen = []
        for c in cands:  # supressão de não-máximos ao longo de x
            if all(abs(c[1] - k[1]) >= dw * 0.7 for k in chosen):
                chosen.append(c)
        chosen.sort(key=lambda c: c[1])
        # mantém só a sequência contígua mais longa (passo ~ largura do dígito)
        runs, cur = [], [chosen[0]]
        for c in chosen[1:]:
            if c[1] - cur[-1][1] <= dw * 1.35:
                cur.append(c)
            else:
                runs.append(cur)
                cur = [c]
        runs.append(cur)
        run = max(runs, key=len)
        if len(run) < 2:
            return None, 0.0
        return int("".join(c[3] for c in run)), min(c[0] for c in run)
