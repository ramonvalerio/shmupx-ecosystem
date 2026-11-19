"""Calibração do HUD e leitura de dígitos por template matching em tons de cinza."""
import json
import os

import cv2
import numpy as np


# elementos do HUD que podem ser contados por ícones (config: "mode": "icons")
ICON_KEYS = ("lives", "bombs")


def color_feature(img_bgr, channel):
    """Imagem de uma característica de cor, para ícones que somem em tons de cinza.
    "blue": quanto o pixel é mais azul que vermelho/verde (ícones azuis sobre fundos variados)."""
    b, g, r = (img_bgr[..., i].astype(np.float32) for i in range(3))
    if channel == "blue":
        return np.clip(b - (g + r) / 2, 0, 255)
    raise SystemExit(f"Canal de cor desconhecido: {channel}")


def _native(path):
    """Barras do sistema: sob um caminho \\?\ do Windows, "labels/x.png" com "/" é recusado (Errno 22)
    e o normpath não converte."""
    return path.replace("/", os.sep) if os.sep != "/" else path


def _imread_color(path):
    path = _native(path)
    if not os.path.isfile(path):
        raise SystemExit(f"Imagem do script não encontrada: {path}. Siga o templates/LEIA-ME.md da versão.")
    img = cv2.imdecode(np.fromfile(path, dtype=np.uint8), cv2.IMREAD_COLOR)
    if img is None:
        raise SystemExit(f"Template não encontrado: {path}")
    return img


def _imread_gray(path):
    path = _native(path)
    if not os.path.isfile(path):
        raise SystemExit(f"Imagem do script não encontrada: {path}. Siga o templates/LEIA-ME.md da versão.")
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
        lives = self.cfg.get("lives") or {}
        # vidas por ícone (ex.: aviõezinhos sob o placar): conta ícones iguais ao template, lado a lado
        ranking = self.cfg.get("ranking_screen") or {}
        # tela de recordes (ex.: "NAME ENTRY"): mostra o placar final quando ele entra no top 10
        self.ranking_label = _imread_gray(os.path.join(self.tdir, ranking["label_template"])) if ranking else None
        self.ranking_digits = {}
        if ranking:
            rdir = os.path.join(self.tdir, ranking["digit_templates"])
            self.ranking_digits = {d: _imread_gray(os.path.join(rdir, f"{d}.png")) for d in "0123456789"
                                   if os.path.exists(os.path.join(rdir, f"{d}.png"))}
        clear = self.cfg.get("clear_screen") or {}
        # tela de final (ex.: "CONGRATULATIONS!"): prova de que a volta foi completada
        self.clear_labels = [_imread_gray(os.path.join(self.tdir, p)) for p in clear.get("label_templates", [])]
        players = self.cfg.get("players") or {}
        # rótulo do 2º jogador (ex.: "2P:"): presente só quando há alguém jogando de 2P
        self.p2_label = _imread_gray(os.path.join(self.tdir, players["p2_label_template"])) if players else None
        # contadores por ícone (vidas, bombas): um por "templates" (ex.: um por personagem jogável) ou o
        # único "template"; cada um vira pares (canal_de_cor_ou_None, imagem) e vale o que casar melhor
        self.icons = {}
        for key in ICON_KEYS:
            c = self.cfg.get(key) or {}
            if c.get("mode") != "icons":
                continue
            views = []  # (id do modelo, canal ou None, imagem)
            for i, rel in enumerate(c.get("templates") or [c["template"]]):
                path = os.path.join(self.tdir, rel)
                if c.get("channel"):
                    # com "channel", o ícone é comparado numa característica de cor em vez do cinza
                    views.append((i, c["channel"], color_feature(_imread_color(path), c["channel"])))
                    if c.get("also_gray"):
                        # compara também em cinza (ícone escuro sobre fundo da mesma cor)
                        views.append((i, None, _imread_gray(path)))
                else:
                    views.append((i, None, _imread_gray(path)))
            self.icons[key] = views

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
        # modelo de ícone escolhido por votação para o vídeo inteiro (ex.: o personagem jogado)
        self.icon_model = {}

    # ---------- calibração ----------
    def calibrate(self, gray_frames, video_h):
        cfg = self.gc
        ref_h = cfg["reference_height"]
        s0 = video_h / ref_h
        ss = cfg["scale_search"]
        scan_frames = gray_frames
        coarse = list(np.arange(s0 * 0.85, s0 * 1.15 + 1e-9, 0.01 * s0))
        best = self._scan_wide(scan_frames, coarse)
        if best[0] < cfg["hud_label_min_score"]:
            coarse = list(np.arange(ss["min"], ss["max"] + 1e-9, ss["coarse_step"]))
            best = self._scan_wide(scan_frames, coarse)
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
            if c.get("mode") == "icons":
                self.offsets[key] = (c["dx"], c["dy"])  # ícones: posição fixa do config
                continue
            # placar alinhado à direita: a casa da esquerda costuma estar vazia; mede pela última
            shift = c["pitch"] * (c["count"] - 1) if c.get("align") == "right" else 0.0
            found = []
            for g in gray_frames:
                gr = cv2.resize(g, None, fx=1 / s, fy=1 / s, interpolation=cv2.INTER_AREA) if abs(s - 1) > 0.01 else g
                if self.label_visible(gr, check_contrast=False) < self.gc["hud_label_min_score"]:
                    continue
                # placar: janela pequena (< meio dígito) para não "pular" para a célula vizinha
                pad = 10 if key == "score" else 22
                d, sc, mg, (fx, fy) = self.read_digit(gr, self.label_pos[0] + c["dx"] + shift, self.label_pos[1] + c["dy"], pad)
                if sc >= 0.8:
                    found.append((fx - shift - self.label_pos[0], fy - self.label_pos[1]))
            if len(found) >= 3:
                self.offsets[key] = (float(np.median([f[0] for f in found])), float(np.median([f[1] for f in found])))
            else:
                self.offsets[key] = (c["dx"], c["dy"])
        return True

    def off(self, key):
        c = self.gc[key]
        return getattr(self, "offsets", {}).get(key, (c["dx"], c["dy"]))

    def _scan_wide(self, frames, scales, sample=10, keep=6):
        """Busca de escala com muitas candidatas. Varrer todas com os 40 quadros levava minutos em
        vídeos grandes (ou com o jogo numa janela menor que a tela gravada): tria com `sample`
        quadros e confirma com todos só as `keep` melhores escalas e suas vizinhas."""
        if len(frames) <= sample:
            return self._scan(frames, scales)
        subset = [frames[i] for i in np.linspace(0, len(frames) - 1, sample).round().astype(int)]
        ranked = self._scan(subset, scales, all_results=True)
        step = scales[1] - scales[0] if len(scales) > 1 else 0.0
        chosen = []
        for _, s, _ in sorted(ranked, reverse=True):
            if all(abs(s - c) > 1.5 * step for c in chosen):
                chosen.append(s)
            if len(chosen) == keep:
                break
        if not chosen:
            return (-1, 1.0, 0, [])
        return self._scan(frames, sorted({round(c + k * step, 6) for c in chosen for k in (-1, 0, 1)}))

    def _scan(self, frames, scales, variants=None, all_results=False):
        """Para cada escala candidata, reduz/amplia a região de busca do quadro para a
        resolução de referência e compara com o template original (mais fiel que escalar o template).
        `all_results`: devolve [(nota, escala, variante)] de toda escala com leitura, não só a melhor."""
        best = (-1, 1.0, 0, [])
        found = []
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
                    found.append((q, s, vi))
                    if q > best[0]:
                        best = (q, s, vi, pos)
        return found if all_results else best

    # ---------- conversão ----------
    def to_ref(self, frame_bgr):
        g = cv2.cvtColor(frame_bgr, cv2.COLOR_BGR2GRAY)
        interp = cv2.INTER_AREA if self.scale > 1 else cv2.INTER_CUBIC
        if abs(self.scale - 1.0) > 0.01:
            g = cv2.resize(g, None, fx=1.0 / self.scale, fy=1.0 / self.scale, interpolation=interp)
        # ícones coloridos: guarda as características de cor do mesmo quadro, na mesma escala
        channels = {(self.gc.get(k) or {}).get("channel") for k in ICON_KEYS} - {None}
        self.features = {}
        if channels:
            color = frame_bgr
            if abs(self.scale - 1.0) > 0.01:
                color = cv2.resize(frame_bgr, None, fx=1.0 / self.scale, fy=1.0 / self.scale, interpolation=interp)
            self.features = {ch: color_feature(color, ch) for ch in channels}
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

    def _blank_cell(self, g, x, y):
        """Casa sem dígito: quase nenhum pixel claro (o placar é branco; o fundo do jogo aparece atrás)."""
        c = self.gc["score"]
        dw, dh = self.gc["digit_size"]
        x, y = int(round(x)), int(round(y))
        cell = g[max(0, y):y + dh, max(0, x):x + dw]
        if cell.size == 0:
            return True
        return float((cell > c.get("white_level", 180)).mean()) < c.get("blank_max_white", 0.06)

    def _read_score_right(self, g, x0, y0):
        """Placar alinhado à direita, sem zeros à esquerda: lê da última casa para a primeira e para
        na primeira casa vazia. Um 0 à esquerda ou uma casa ilegível invalidam a leitura."""
        c = self.gc["score"]
        digits, mins, minm = [], 1.0, 1.0
        x = x0 + c["pitch"] * (c["count"] - 1)
        for _ in range(c["count"]):
            if self._blank_cell(g, x, y0):
                break
            d, sc, mg, (fx, fy) = self.read_digit(g, x, y0, c["pad"])
            digits.append(d)
            mins, minm = min(mins, sc), min(minm, mg)
            x = (fx if abs(fx - x) <= c["pad"] else x) - c["pitch"]
        s = "".join(reversed(digits))
        ok = (bool(s) and mins >= self.gc["digit_min_score"] and minm >= self.gc["digit_min_margin"]
              and not (len(s) > 1 and s[0] == "0"))
        # placares do jogo são sempre múltiplos de um valor (ex.: 100): o resto é leitura errada
        if ok and c.get("multiple_of") and int(s) % c["multiple_of"]:
            ok = False
        return s or "0", ok, mins, minm

    def read_score(self, g):
        c = self.gc["score"]
        lx, ly = self.label_pos
        dx, dy = self.off("score")
        x0, y0 = lx + dx, ly + dy
        if c.get("align") == "right":
            return self._read_score_right(g, x0, y0)
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

    def clear_visible(self, g):
        """Nota (NCC) da melhor imagem da tela de final dentro da região do config."""
        c = self.gc["clear_screen"]
        x0, y0, x1, y1 = _region(g.shape, c["label_search_region"])
        return max((_best_match(g[y0:y1, x0:x1], t)[0] for t in self.gc.clear_labels), default=-1.0)

    def ranking_visible(self, g):
        """Nota (NCC) do título da tela de recordes dentro da região do config."""
        c = self.gc["ranking_screen"]
        x0, y0, x1, y1 = _region(g.shape, c["label_search_region"])
        return _best_match(g[y0:y1, x0:x1], self.gc.ranking_label)[0]

    def read_ranking_numbers(self, g):
        """Todos os números da tela de recordes (busca livre de dígitos, agrupados por linha)."""
        c = self.gc["ranking_screen"]
        x0, y0, x1, y1 = _region(g.shape, c["number_region"])
        area = g[y0:y1, x0:x1]
        threshold = c.get("digit_min_score", 0.75)
        cands = []
        for d, t in self.gc.ranking_digits.items():
            if area.shape[0] < t.shape[0] or area.shape[1] < t.shape[1]:
                continue
            m = cv2.matchTemplate(area, t, cv2.TM_CCOEFF_NORMED)
            ys, xs = np.where(m > threshold)
            cands.extend((float(m[y, x]), int(x), int(y), d) for y, x in zip(ys, xs))
        if not cands:
            return []
        th, tw = next(iter(self.gc.ranking_digits.values())).shape
        cands.sort(reverse=True)
        kept = []
        for cand in cands:  # supressão de não-máximos: um dígito por posição
            if all(abs(cand[1] - k[1]) >= tw * 0.7 or abs(cand[2] - k[2]) >= th * 0.5 for k in kept):
                kept.append(cand)
        kept.sort(key=lambda k: (k[2], k[1]))
        rows = []
        for k in kept:
            if rows and abs(k[2] - rows[-1][-1][2]) <= 5:
                rows[-1].append(k)
            else:
                rows.append([k])
        numbers = []
        for row in rows:
            row.sort(key=lambda k: k[1])
            run = [row[0]]
            for k in row[1:] + [None]:
                if k is not None and k[1] - run[-1][1] <= tw * 1.4:
                    run.append(k)
                    continue
                if len(run) >= 2:
                    numbers.append(int("".join(r[3] for r in run)))
                if k is not None:
                    run = [k]
        return numbers

    def p2_visible(self, g):
        """Nota (NCC) do rótulo do 2º jogador na posição do config, relativa ao rótulo do 1º."""
        c = self.gc["players"]
        t = self.gc.p2_label
        lx, ly = self.label_pos
        x, y, pad = int(round(lx + c["dx"])), int(round(ly + c["dy"])), c.get("pad", 6)
        # na horizontal a busca é mais larga: vídeos esticados/espremidos mudam a distância entre os rótulos
        pad_x = c.get("pad_x", pad)
        sc, _ = _best_match(g[max(0, y - pad):y + t.shape[0] + pad, max(0, x - pad_x):x + t.shape[1] + pad_x], t)
        return sc

    def choose_icon_models(self, frames_bgr):
        """Com vários modelos de ícone (um por personagem), vota nos quadros da calibração qual casa
        com o 1º ícone e passa a usar só ele: testar todos em cada quadro deixa algum casar com o
        fundo. Sem votos (nenhum ícone visível), continua testando todos."""
        for key, views in self.gc.icons.items():
            if len({i for i, _, _ in views}) < 2:
                continue
            votes = {}
            for frame in frames_bgr:
                g = self.to_ref(frame)
                if self.label_visible(g, check_contrast=False) < self.gc["hud_label_min_score"]:
                    continue
                count, _, _, model = self._count_icons(g, key, first_only=True)
                if count:
                    votes[model] = votes.get(model, 0) + 1
            if votes:
                # fica com todos os modelos bem votados: o mesmo personagem pode ter mais de um
                # recorte (ex.: de vídeos diferentes), e um sozinho pode casar mal em parte do vídeo
                top = max(votes.values())
                self.icon_model[key] = {model for model, n in votes.items() if n >= max(2, top * 0.25)} or {max(votes, key=votes.get)}
        return self.icon_model

    def count_icons(self, g, key):
        count, ok, weakest, _ = self._count_icons(g, key)
        return count, ok, weakest

    def _count_icons(self, g, key, first_only=False):
        """Quantos ícones iguais ao template há lado a lado a partir da posição do config."""
        c = self.gc[key]
        # (id do modelo, imagem, template) a comparar em cada posição; vale a melhor nota (features: de to_ref).
        # Os ícones de uma fileira são iguais (mesmo personagem): depois do 1º, só o modelo que casou com ele.
        views = [(i, self.features[ch] if ch else g, t) for i, ch, t in self.gc.icons[key]
                 if key not in self.icon_model or i in self.icon_model[key]]
        lx, ly = self.label_pos
        dx, dy = self.off(key)
        pad, threshold = c.get("pad", 3), c.get("min_score", 0.7)
        # vídeos esticados ou com escala levemente errada: o 2º ícone pode estar a pitch*(1 ± tolerância)
        # do 1º; daí em diante vale o passo medido entre os dois (±1 px)
        tolerance = max(1, int(round(c["pitch"] * c.get("pitch_tolerance", 0.0))))
        count, weakest, doubtful, first_model = 0, 1.0, False, None
        prev, step = None, None  # os ícones de verdade ficam exatamente a um passo do anterior
        for _ in range(1 if first_only else c.get("max", 9)):
            if prev is None:
                x, y, px, py = int(round(lx + dx)), int(round(ly + dy)), pad, c.get("pad_first_y", pad)
            else:
                x, y = int(round(prev[0] + (step or c["pitch"]))), prev[1]
                px, py = (1 if step else tolerance), 1
            ox, oy = max(0, x - px), max(0, y - py)
            sc, (mx, my), model = max(
                (*_best_match(img[oy:y + tt.shape[0] + py, ox:x + tt.shape[1] + px], tt), i)
                for i, img, tt in views)
            found = (ox + mx, oy + my)
            if sc < threshold:
                doubtful = sc > threshold - c.get("doubt_margin", 0.15)  # quase um ícone: leitura incerta
                break
            if prev is None:
                views = [view for view in views if view[0] == model]
                first_model = model
            if prev is not None and step is None:
                step = found[0] - prev[0]
            count += 1
            weakest = min(weakest, sc)
            prev = found
        return count, not doubtful, weakest, first_model

    def read_small(self, g, key):
        c = self.gc[key]
        if c.get("mode") == "icons":
            return self.count_icons(g, key)
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
