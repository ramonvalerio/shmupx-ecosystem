#!/usr/bin/env python3
"""ShmupX offline – verifica uma gameplay a partir de um arquivo de vídeo local.

Exemplo:
    python analyze.py gameplay.mp4 --game earthion --player "Fulano" --out result.json
"""
import argparse
import json
import os
import sys
import time

from contextlib import redirect_stdout

from .registry import VALIDATORS, get_validator


def main(argv=None):
    # Contrato UTF-8 também em pipes do Windows (nomes de jogador e logs Unicode).
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description="Verificação offline de gameplay de shmup (placar, vidas, continues).")
    ap.add_argument("video", nargs="?", help="arquivo de vídeo local (MP4, MKV, WEBM...)")
    ap.add_argument("--game", help="ID do validador do jogo")
    ap.add_argument("--list-games", action="store_true", help="lista validadores disponíveis")
    ap.add_argument("--format", choices=("legacy", "client-v1"), default="legacy",
                    help="formato da saída (padrão: legacy)")
    ap.add_argument("--player", help="nome do jogador/canal")
    ap.add_argument("--url", default=None, help="URL do vídeo (opcional, só vai para o JSON)")
    ap.add_argument("--game-mode", default=None, help="ex.: 1cc. Se omitido: '1cc' sem continues, senão 'Ncc'")
    ap.add_argument("--out", default=None, help="arquivo JSON de saída (padrão: imprime na tela)")
    ap.add_argument("--debug-dir", default=None, help="pasta para salvar leituras e recortes de depuração")
    ap.add_argument("--grok", action="store_true", help="confere o placar final com a API do Grok (precisa XAI_API_KEY)")
    ap.add_argument("--grok-model", default="grok-4.7")
    ap.add_argument("--grok-always", action="store_true", help="manda o recorte final ao Grok mesmo com alta confiança")
    ap.add_argument("--brl-rate", type=float, default=5.05, help="cotação US$→R$ (padrão 5.05)")
    ap.add_argument("--price-in", type=float, default=2.00, help="US$ por 1M tokens de entrada")
    ap.add_argument("--price-out", type=float, default=6.00, help="US$ por 1M tokens de saída")
    ap.add_argument("--quiet", action="store_true")
    a = ap.parse_args(argv)
    if a.list_games:
        print(json.dumps([{"id": s.game_id, "name": s.name, "version": s.version}
                          for s in VALIDATORS], ensure_ascii=False, indent=2))
        return
    if not a.video or not a.game or not a.player:
        ap.error("VIDEO, --game e --player são obrigatórios para analisar")
    if a.grok_always and not a.grok:
        ap.error("--grok-always exige --grok (autoriza acesso à API externa)")
    try:
        spec = get_validator(a.game)
    except ValueError as exc:
        ap.error(str(exc))
    a.validator_version = spec.version

    if not os.path.isfile(a.video):
        sys.exit(f"Arquivo não encontrado: {a.video}")
    t0 = time.time()
    gk = None
    if a.grok:  # valida a chave antes de gastar minutos analisando
        from shmupx.grok import Grok
        gk = Grok(model=a.grok_model)
    an = spec.create(a.video, debug_dir=a.debug_dir, quiet=a.quiet)
    try:
        # stdout é reservado ao JSON; inclusive logs de scripts futuros vão para stderr.
        with redirect_stdout(sys.stderr):
            txt = _analyze(a, an, gk, t0)
        print(txt)
    finally:
        an.v.cap.release()


def _analyze(a, an, gk, t0):
    from .formatting import hms

    r = an.run()

    total_tokens = None
    if gk is not None:
        if r["confidence"] == "low" or a.grok_always:
            t = an.final_crop_time
            fr = an.v.frame_at(t) if t is not None else None
            if fr is not None:
                if r["results"] and r["hi"] == r["results"]["total"]:
                    crop, what = fr, "the 'Total Score' number on this RESULTS screen"
                else:
                    crop, what = an.hud.score_crop(fr), "the 8-digit SCORE"
                val = gk.read_number(crop, what)
                print(f"[grok] leitura: {val} (local: {r['hi']})")
                if val is not None and val != r["hi"] and r["confidence"] == "low":
                    r["observation"] = ((r["observation"] or "") + f" Placar final confirmado pelo Grok: {val} "
                                        f"(leitura local era {r['hi']}).").strip()
                    r["hi"] = val
                elif val is not None and val != r["hi"]:
                    r["observation"] = ((r["observation"] or "") + f" Atenção: o Grok leu {val} no placar final, "
                                        f"diferente da leitura local {r['hi']} (mantida, alta confiança).").strip()
        total_tokens = gk.total_tokens
        usd = gk.cost_usd(a.price_in, a.price_out)
        print(f"[grok] chamadas={gk.calls} tokens={gk.total_tokens} custo≈US${usd:.4f} ≈ R${usd * a.brl_rate:.4f}")

    mode = a.game_mode or ("1cc" if r["continues"] == 0 else f"{r['continues'] + 1}cc")
    out = {a.game: {
        "player": a.player,
        "url": a.url,
        "game-mode": mode,
        "duration": hms(an.v.duration),
        "fps-analysed": an.v.frames_read,
        "hi-score": r["hi"],
        "continues": r["continues"],
        "lives-lost": r["lives_lost"],
        "game-over": r["game_over"],
        "attempts": r["attempts"],
        "observation": r["observation"],
        "total_tokens": total_tokens,
    }}
    if a.format == "client-v1":
        from .report import client_report
        out = client_report(a.game, a.validator_version, r, out[a.game])
    txt = json.dumps(out, ensure_ascii=False, indent=2)
    if a.out:
        with open(a.out, "w", encoding="utf-8") as f:
            f.write(txt + "\n")
    if not a.quiet:
        print(f"[info] confiança do placar: {r['confidence']}; mortes: {r['deaths']}; tempo: {time.time() - t0:.0f}s",
              file=sys.stderr)

    return txt


if __name__ == "__main__":
    main()
