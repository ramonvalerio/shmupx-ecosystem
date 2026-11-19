"""Analisador genérico por HUD: amostragem, re-checagem densa, regras de placar, vidas, continues
e attempts. O que é de cada jogo vem do script (config.json e templates/ da versão)."""
import os
import time
from collections import Counter

import cv2
import numpy as np

from ..hud import GameConfig, Hud
from ..video import Video


def hms(t):
    t = int(round(t))
    return f"{t // 3600:02d}:{t % 3600 // 60:02d}:{t % 60:02d}"


def log(msg, quiet=False):
    if not quiet:
        print(msg, flush=True)


class Analyzer:
    def __init__(self, gc, video_path, debug_dir=None, quiet=False):
        self.gc = gc
        self.v = Video(video_path)
        self.hud = Hud(gc)
        self.debug_dir = debug_dir
        self.quiet = quiet
        self.samples = []      # leituras da passada base
        self.low_conf = []     # (motivo, recorte BGR) para Grok/debug
        self.notes = []
        if debug_dir:
            os.makedirs(debug_dir, exist_ok=True)

    # ------------------------------------------------------------------ leitura de um quadro
    def read_frame(self, t, frame, want_results=True):
        g = self.hud.to_ref(frame)
        mean = float(g.mean())
        r = {"t": t, "mean": mean}
        lab = self.hud.label_visible(g)
        r["hud"] = lab >= self.gc["flash"]["min_label_score"] and mean < self.gc["flash"]["max_mean"]
        r["label"] = lab
        if r["hud"]:
            s, ok, sc, mg = self.hud.read_score(g)
            r["score"] = int(s) if ok else None
            r["score_raw"], r["score_conf"] = s, (sc, mg)
            lv, lok, _ = self.hud.read_small(g, "lives")
            r["lives"] = lv if lok else None
            if self.gc.get("bombs"):
                b, bok, _ = self.hud.read_small(g, "bombs")
                r["bombs"] = b if bok else None
            if self.gc.get("players"):
                r["p2"] = self.hud.p2_visible(g) >= self.gc["players"].get("min_score", 0.7)
            if self.gc.get("stage"):  # nem todo HUD mostra a fase
                st, sok, _ = self.hud.read_small(g, "stage")
                r["stage"] = st if sok else None
        elif mean < self.gc["flash"]["max_mean"] and self.hud.alt_label_visible(g) >= max(0.8, self.gc["flash"]["min_label_score"]):
            # HUD de chefe: sem placar, mas vidas/fase legíveis
            r["boss"] = True
            lv, lok, _ = self.hud.read_small(g, "lives")
            r["lives"] = lv if lok else None
        elif want_results and self.gc.get("ranking_screen") and \
                self.hud.ranking_visible(g) >= self.gc["ranking_screen"].get("label_min_score", 0.7):
            r["ranking"] = True
        elif want_results and self.gc.get("clear_screen") and \
                self.hud.clear_visible(g) >= self.gc["clear_screen"].get("label_min_score", 0.7):
            r["clear"] = True
        elif want_results and self.gc.get("results"):
            lab = self.hud.find_results_label(g)
            if lab[0] >= self.gc["results"]["label_min_score"]:
                r["results"] = True
                total, conf = self.hud.read_results_total(g, lab)
                r["total"], r["total_conf"] = total, conf
        return r

    # ------------------------------------------------------------------ etapas
    def calibrate(self):
        dur = self.v.duration
        frames, color = [], []
        for k in range(1, 41):
            f = self.v.frame_at(dur * k / 41.0)
            if f is not None:
                color.append(f)
                frames.append(cv2.cvtColor(f, cv2.COLOR_BGR2GRAY).astype(np.float32))
        if not self.hud.calibrate(frames, self.v.height):
            raise SystemExit("HUD não encontrado. O vídeo é mesmo deste jogo? Veja README (limitações).")
        chosen = self.hud.choose_icon_models(color)
        if chosen:
            names = {key: [(self.gc[key].get("templates") or [self.gc[key].get("template")])[i] for i in sorted(models)]
                     for key, models in chosen.items()}
            log(f"[calibração] ícones escolhidos: {names}", self.quiet)
        log(f"[calibração] variante={self.hud.variant['name']} escala={self.hud.scale:.3f} "
            f"rótulo em {tuple(round(x) for x in self.hud.label_pos)} (ref 720p), ncc={self.hud.label_score:.2f}, "
            f"offsets={ {k: tuple(round(x) for x in v) for k, v in self.hud.offsets.items()} }", self.quiet)

    def base_pass(self):
        step = self.gc["sample_interval_s"]
        t0 = time.time()
        for t, frame in self.v.sequential(step):
            self.samples.append(self.read_frame(t, frame))
            if len(self.samples) % 200 == 0:
                log(f"  {hms(t)} analisado ({time.time() - t0:.0f}s)", self.quiet)
        log(f"[passada base] {len(self.samples)} quadros, duração {hms(self.v.duration)}", self.quiet)

    def dense(self, t0, t1, fps, want_results=False):
        return [self.read_frame(t, f, want_results) for t, f in self.v.window(t0, t1, fps)]

    # ------------------------------------------------------------------ vidas
    def lives_timeline(self, start_t):
        # HUDs em que explosões cobrem as vidas pedem uma confirmação mais longa no config
        return self.icon_timeline("lives", start_t, self.gc.get("lives_confirm_samples", 3),
                                  self.gc.get("lives_confirm_s", 1.9))

    def bombs_timeline(self, start_t):
        c = self.gc["bombs"]
        return self.icon_timeline("bombs", start_t, c.get("confirm_samples", 3), c.get("confirm_s", 1.0))

    def icon_timeline(self, key, start_t, min_count, min_span):
        """Trechos [início, fim, valor] de um contador do HUD (vidas, bombas), re-checando as mudanças."""
        base = [(r["t"], r[key]) for r in self.samples if r["t"] >= start_t and (r.get("hud") or r.get("boss")) and r.get(key) is not None]
        if not base:
            return []
        pts = list(base)
        fps = self.gc["lives_dense_fps"]
        # re-checa densamente: toda mudança e todo buraco sem leitura > 6s
        for (ta, va), (tb, vb) in zip(base, base[1:]):
            if va != vb or tb - ta > 6.0:
                for r in self.dense(ta, tb, fps):
                    if (r.get("hud") or r.get("boss")) and r.get(key) is not None:
                        pts.append((r["t"], r[key]))
        pts.sort()
        # runs: um valor só vale se aparecer em leituras seguidas suficientes (ou por tempo suficiente)
        runs = []
        i = 0
        while i < len(pts):
            j = i
            while j + 1 < len(pts) and pts[j + 1][1] == pts[i][1]:
                j += 1
            span, cnt = pts[j][0] - pts[i][0], j - i + 1
            if cnt >= min_count or span >= min_span:
                if runs and runs[-1][2] == pts[i][1]:
                    runs[-1][1] = pts[j][0]
                else:
                    runs.append([pts[i][0], pts[j][0], pts[i][1]])
            i = j + 1
        return runs

    # ------------------------------------------------------------------ placar
    def score_series(self):
        """Leituras confiáveis + validação monotônica. Devolve (aceitas, eventos_de_queda)."""
        reads = [(r["t"], r["score"], r.get("stage")) for r in self.samples if r.get("hud") and r.get("score") is not None]
        acc, drops = [], []
        n_conf = self.gc["restart"]["confirm_samples"]
        ratio = self.gc["restart"]["score_drop_ratio"]
        i = 0
        while i < len(reads):
            t, s, st = reads[i]
            if not acc or s >= acc[-1][1]:
                # rejeita saltos absurdos para cima que não se confirmam no quadro seguinte
                if acc and i + 1 < len(reads) and s > reads[i + 1][1] and reads[i + 1][1] >= acc[-1][1]:
                    i += 1
                    continue
                acc.append((t, s, st))
            else:
                nxt = [x for x in reads[i:i + n_conf]]
                consistent = len(nxt) == n_conf and all(nxt[k][1] <= nxt[k + 1][1] for k in range(n_conf - 1)) \
                    and all(x[1] < acc[-1][1] * ratio for x in nxt)
                if consistent:
                    drops.append((t, acc[-1][1], s, st))
                    acc.append((t, s, st))
            i += 1
        return acc, drops

    def final_hud_score(self, start_t):
        vis = [r for r in self.samples if r["t"] >= start_t and r.get("hud")]
        if not vis:
            return None, None, False
        t_last = vis[-1]["t"]
        fv = self.gc["final_vote"]
        reads = self.dense(t_last - fv["window_s"], t_last + self.gc["sample_interval_s"] + 1, fv["dense_fps"])
        vals = [(r["t"], r["score"]) for r in reads if r.get("hud") and r.get("score") is not None]
        best = self._last_stable(vals, fv["min_consecutive"])
        if best:
            return best[0], best[1], True
        # sem maioria estável: usa a última leitura aceita na passada base (baixa confiança)
        acc = [x for x in self.score_series()[0] if x[0] >= start_t]
        if acc:
            return acc[-1][1], acc[-1][0], False
        return None, None, False

    @staticmethod
    def _last_stable(vals, min_run):
        """Último valor que aparece em >= min_run leituras consecutivas (voto de maioria)."""
        best = None
        i = 0
        while i < len(vals):
            j = i
            while j + 1 < len(vals) and vals[j + 1][1] == vals[i][1]:
                j += 1
            if j - i + 1 >= min_run:
                if best is None or vals[i][1] >= best[0]:
                    best = (vals[i][1], vals[j][0], j - i + 1)
            i = j + 1
        if best is None and vals:
            # fallback: maioria simples
            c = Counter(v for _, v in vals).most_common(1)[0]
            if c[1] >= min_run:
                t = max(t for t, v in vals if v == c[0])
                best = (c[0], t, c[1])
        return best

    def results_total(self, start_t):
        res = [r for r in self.samples if r["t"] >= start_t and r.get("results")]
        if not res:
            return None
        t0, t1 = res[0]["t"], res[-1]["t"]
        rf = self.gc["results"]
        reads = self.dense(t0 - 1, t1 + self.gc["sample_interval_s"] + 1, rf["dense_fps"], want_results=True)
        vals = [(r["t"], r["total"]) for r in reads if r.get("results") and r.get("total")]
        best = self._last_stable(vals, self.gc["final_vote"]["min_consecutive"])
        if best:
            return {"total": best[0], "t": best[1], "votes": best[2], "stable": True}
        if vals:
            return {"total": vals[-1][1], "t": vals[-1][0], "votes": 1, "stable": False}
        return None

    def ranking_score(self, start_t, hud_score):
        """Placar final na tela de recordes: o menor número >= placar do HUD e até `max_gain` acima
        dele (os outros são recordes antigos da máquina), estável em leituras seguidas."""
        cfg = self.gc.get("ranking_screen")
        if not cfg or hud_score is None:
            return None
        seen = [r["t"] for r in self.samples if r["t"] >= start_t and r.get("ranking")]
        if not seen:
            return None
        t0, t1 = seen[0], seen[-1]
        votes = []
        for t, frame in self.v.window(t0 - 1, t1 + self.gc["sample_interval_s"] + 1, cfg.get("dense_fps", 4)):
            g = self.hud.to_ref(frame)
            if self.hud.ranking_visible(g) < cfg.get("label_min_score", 0.7):
                continue
            fits = [n for n in self.hud.read_ranking_numbers(g) if hud_score <= n <= hud_score + cfg.get("max_gain", 100000)]
            if fits:
                votes.append((t, min(fits)))
        best = self._last_stable(votes, cfg.get("min_consecutive", 3))
        return {"total": best[0], "t": best[1], "votes": best[2]} if best else None

    def players(self):
        """1 ou 2: há um 2º jogador se o rótulo dele aparece numa parte relevante da gameplay
        (alguns quadros soltos podem ser só a tela de "entrar" ou um falso positivo)."""
        cfg = self.gc.get("players")
        if not cfg:
            return 1
        hud = [r for r in self.samples if r.get("hud")]
        p2 = sum(1 for r in hud if r.get("p2"))
        return 2 if hud and p2 >= max(cfg.get("min_samples", 10), cfg.get("min_fraction", 0.05) * len(hud)) else 1

    # ------------------------------------------------------------------ principal
    def run(self):
        self.calibrate()
        self.base_pass()
        acc, drops = self.score_series()

        # reinícios: queda de placar com fase 1 (ou fase volta para 1)
        restarts, continues_by_drop = [], []
        for t, prev, new, st in drops:
            if st in (None, 1):
                restarts.append(t)
            else:
                continues_by_drop.append(t)
        stage_reads = [(r["t"], r["stage"]) for r in self.samples if r.get("hud") and r.get("stage") is not None]
        for (ta, sa), (tb, sb), (tc, sc) in zip(stage_reads, stage_reads[1:], stage_reads[2:]):
            if sa > 1 and sb == 1 and sc == 1 and not any(abs(tb - x) < 60 for x in restarts):
                restarts.append(tb)
        restarts.sort()
        start_t = restarts[-1] - 1 if restarts else 0.0
        log(f"[regras] reinícios={len(restarts)} {[hms(x) for x in restarts]}", self.quiet)

        runs = self.lives_timeline(start_t)
        lives_cfg = self.gc.get("lives") or {}
        # HUD de reservas (comum em shmups): 2 ícones = 3 vidas, 0 = jogando a última. Sair do 0 para
        # menos que o inicial é vida extra; voltar ao inicial (ou mais) é continue.
        reserve = lives_cfg.get("counts_reserve", False)
        start_lives = lives_cfg.get("start", 1)
        lives_lost, continues, deaths = 0, 0, []
        for a, b in zip(runs, runs[1:]):
            if b[2] < a[2]:
                lives_lost += a[2] - b[2]
                deaths.append((b[0], a[2], b[2]))
            elif a[2] == 0 and b[2] > 0 and (not reserve or b[2] >= start_lives):
                continues += 1
                lives_lost += 1
                deaths.append((b[0], 0, b[2]))
        continues += sum(1 for t in continues_by_drop if t >= start_t)
        log("[vidas] " + " → ".join(f"{r[2]}@{hms(r[0])}" for r in runs), self.quiet)

        bombs_used = None
        if self.gc.get("bombs"):
            # bomba gasta = o estoque caiu sem morte por perto (ao morrer, o jogo repõe as bombas)
            window = self.gc["bombs"].get("death_window_s", 5.0)
            death_times = [t for t, _, _ in deaths]
            bomb_runs = self.bombs_timeline(start_t)
            # queda que volta ao nível anterior em pouco tempo é algo cobrindo os ícones, não bomba usada
            flicker = self.gc["bombs"].get("flicker_s", 3.0)
            bomb_runs = [r for i, r in enumerate(bomb_runs)
                         if not (0 < i < len(bomb_runs) - 1 and r[1] - r[0] < flicker
                                 and bomb_runs[i - 1][2] > r[2] <= bomb_runs[i + 1][2]
                                 and bomb_runs[i + 1][2] >= bomb_runs[i - 1][2])]
            bombs_used = sum(a[2] - b[2] for a, b in zip(bomb_runs, bomb_runs[1:])
                             if b[2] < a[2] and not any(abs(b[0] - t) <= window for t in death_times))
            log("[bombas] " + " → ".join(f"{r[2]}@{hms(r[0])}" for r in bomb_runs), self.quiet)

        players = self.players()
        res = self.results_total(start_t)
        clear_t = next((r["t"] for r in self.samples if r["t"] >= start_t and r.get("clear")), None)
        # volta completa: a tela de final apareceu (ou a de resultado, que só vem depois de zerar)
        cleared = clear_t is not None or res is not None
        hud_score, hud_t, hud_stable = self.final_hud_score(start_t)
        ranking = self.ranking_score(start_t, hud_score)
        last_lives = runs[-1][2] if runs else None

        obs = []
        hi, conf = None, "high"
        if res and (hud_score is None or res["total"] >= hud_score):
            hi = res["total"]
            if not res["stable"]:
                conf = "low"
                obs.append("Leitura do Total Score da tela RESULTS sem maioria estável (baixa confiança).")
            log(f"[RESULTS] Total Score {hi} em {hms(res['t'])} ({res['votes']} leituras iguais)", self.quiet)
        elif ranking:
            # o último placar do HUD pode ficar para trás (pontos da explosão final); a tela de
            # recordes mostra o valor definitivo
            hi = ranking["total"]
            obs.append(f"Placar final lido na tela de recordes em {hms(ranking['t'])} "
                       f"({ranking['votes']} leituras iguais); o último placar estável do HUD era {hud_score}.")
        else:
            hi = hud_score
            if not hud_stable:
                conf = "low"
            if hud_score is not None:
                obs.append(f"O vídeo terminou antes da tela RESULTS (contagem de bônus e Total Score); "
                           f"o hi-score é o último placar estável do HUD, capturado em {hms(hud_t)}.")
            if res:
                obs.append("A tela RESULTS foi detectada, mas o total lido era menor que o placar do HUD e foi descartado.")
        if reserve:
            # Com 0 reservas o jogador ainda está vivo: só é game over se o HUD sumir de vez antes
            # do fim do vídeo (morreu na última vida) sem a tela de final. Essa morte também conta.
            hud_times = [r["t"] for r in self.samples if r.get("hud") or r.get("boss")]
            hud_gone = bool(hud_times) and self.v.duration - hud_times[-1] >= lives_cfg.get("game_over_gap_s", 5.0)
            game_over = bool(not cleared and last_lives == 0 and hud_gone)
            if game_over:
                lives_lost += 1
                deaths.append((hud_times[-1], 0, -1))
        else:
            game_over = bool(not cleared and last_lives == 0)
        if clear_t is not None:
            obs.append(f"Jogo zerado: a tela de final apareceu em {hms(clear_t)}.")
        if game_over and reserve:
            obs.append("O jogador morreu na última vida (sem reservas) e o jogo terminou sem a tela de final (game over).")
        elif game_over:
            obs.append("O jogo terminou com 0 vidas restantes e sem tela RESULTS (provável game over).")
        if restarts:
            obs.append(f"Houve {len(restarts)} reinício(s) desde a fase 1; só a última tentativa foi considerada.")
        if players > 1:
            obs.append("Partida com 2 jogadores (cooperativa): o placar e as vidas lidos são só do 1P.")
        if conf == "low":
            obs.append("Placar final lido com baixa confiança; recomenda-se conferência manual (ou --grok).")
        self.final_crop_time = res["t"] if (res and hi == res["total"]) else ranking["t"] if (ranking and hi == ranking["total"]) else hud_t
        self.result = {
            "hi": hi, "confidence": conf, "continues": continues, "lives_lost": lives_lost,
            "game_over": game_over, "attempts": len(restarts), "players": players, "cleared": cleared, "bombs_used": bombs_used, "observation": " ".join(obs) if obs else None,
            "deaths": [(hms(t), a, b) for t, a, b in deaths], "lives_runs": runs,
            "results": res, "hud_final": (hud_score, hud_t, hud_stable),
        }
        if self.debug_dir:
            self._dump_debug()
        return self.result

    def _dump_debug(self):
        import json
        with open(os.path.join(self.debug_dir, "samples.json"), "w", encoding="utf-8") as f:
            json.dump(self.samples, f, default=lambda o: o if not isinstance(o, np.generic) else o.item())
        t = self.final_crop_time
        if t is not None:
            fr = self.v.frame_at(t)
            if fr is not None:
                cv2.imwrite(os.path.join(self.debug_dir, "final_frame.png"), fr)
                cv2.imwrite(os.path.join(self.debug_dir, "final_score_crop.png"), self.hud.score_crop(fr))


def create_analyzer(script_dir, video_path, *, debug_dir=None, quiet=False):
    """Cria o analisador com a calibração e as imagens da pasta do script (games/<jogo>/<versão>/)."""
    return Analyzer(GameConfig(script_dir), video_path, debug_dir=debug_dir, quiet=quiet)
