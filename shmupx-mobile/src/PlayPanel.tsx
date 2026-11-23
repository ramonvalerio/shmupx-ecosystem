import { useState } from "react";
import { useI18n } from "@client/i18n";
import { readMisterPlayer } from "@client/data/misterPlayer";
import type { GameMode, GameRoom } from "@client/types/room";
import type { Kind, Platform, PlayPreset } from "./prefs";
import { PLATFORM_LABEL, PLATFORM_MARK, SessionBar, modeFor, platformsFor, roomModes, useMisterCompatible, usePlay } from "./play";
import { useBackLayer } from "./back";

export { useMisterCompatible } from "./play";

/**
 * Jogar pela sala: Play Training (sem ficha) ou Play Ranked (gasta 1 ficha). A folha já vem com a
 * pré-configuração da tela inicial (plataforma e modo); é só tocar em Jogar.
 */
export default function PlayPanel({ room, preset, onSettings }: { room: GameRoom; preset: PlayPreset; onSettings: () => void }) {
  const { t } = useI18n();
  const play = usePlay();
  const misterOk = useMisterCompatible(room);
  const platforms = platformsFor(room, misterOk);
  const modes = roomModes(room);
  const player = readMisterPlayer();
  const [sheet, setSheet] = useState<{ kind: Kind; mode: GameMode; platform: Platform | null } | null>(null);
  useBackLayer(sheet !== null, () => setSheet(null));

  function open(kind: Kind) {
    play.clearError();
    const usable = (p: Platform) => play.availability(room, p, kind).ok;
    // A da pré-configuração, se a sala tem; senão uma que dá para usar agora.
    const preferred = platforms.includes(preset.platform) ? preset.platform : platforms.find(usable) ?? platforms[0] ?? null;
    setSheet({ kind, mode: modeFor(room, preset.mode), platform: preferred });
  }

  async function confirm() {
    if (!sheet?.platform) return;
    if (sheet.platform === "mister" && !player) { setSheet(null); onSettings(); return; }
    if (await play.start(room, sheet.kind, sheet.mode, sheet.platform)) setSheet(null);
  }

  if (platforms.length === 0) return null;

  const mine = play.session?.room.id === room.id || play.external?.gameId === room.id;
  const error = play.error && (play.error.roomId === room.id || play.error.roomId === null) ? play.error.message : null;
  const sheetPlatform = sheet?.platform ?? null;
  const sheetAvailability = sheetPlatform && sheet ? play.availability(room, sheetPlatform, sheet.kind) : null;
  const canPlay = Boolean(sheetPlatform && sheetAvailability && (sheetAvailability.ok || (sheetPlatform === "mister" && !player)));

  return (
    <section className="lite-play" aria-label={t("Jogar")}>
      {play.session || play.external ? <SessionBar /> : (
        <div className="lite-play-buttons">
          <button type="button" className="lite-play-training" onClick={() => open("training")}>
            <strong>{t("Play Training")}</strong><small>{t("sem ficha")}</small>
          </button>
          <button type="button" className="lite-play-ranked" onClick={() => open("ranked")}>
            <strong>{t("Play Ranked")}</strong><small>{t("gasta 1 ficha")}</small>
          </button>
        </div>
      )}
      {(play.session || play.external) && !mine && <p className="lite-muted">{t("Pare o jogo aberto para jogar esta sala.")}</p>}
      {error && !sheet && <p className="lite-error" role="alert">{t(error)}</p>}

      {sheet && (
        <div className="lite-sheet-backdrop" onClick={(event) => { if (event.target === event.currentTarget && !play.busy) setSheet(null); }}>
          <div className="lite-sheet" role="dialog" aria-modal="true" aria-labelledby="lite-sheet-title">
            <div className="lite-sheet-grip" aria-hidden="true" />
            <header className="lite-sheet-header">
              <div>
                <h2 id="lite-sheet-title">{t(sheet.kind === "training" ? "Play Training" : "Play Ranked")}</h2>
                <p>{room.title} · {t(sheet.kind === "training" ? "sem ficha nem ranking" : "gasta 1 ficha e vale ranking")}</p>
              </div>
              <button type="button" className="lite-icon-button" aria-label={t("Fechar")} disabled={play.busy} onClick={() => setSheet(null)}>✕</button>
            </header>

            <div className="lite-sheet-body">
              <h3 className="lite-sheet-label">{t("Modo de jogo")}</h3>
              <div className="lite-mode-chips" role="radiogroup" aria-label={t("Modo de jogo")}>
                {modes.map((m) => (
                  <button key={m.id} type="button" role="radio" aria-checked={sheet.mode === m.id} className="lite-chip" onClick={() => setSheet({ ...sheet, mode: m.id })}>
                    <strong>{m.label}</strong><small>{t(m.detail)}</small>
                  </button>
                ))}
              </div>

              <h3 className="lite-sheet-label">{t("Plataforma")}</h3>
              <div className="lite-option-list" role="radiogroup" aria-label={t("Plataforma")}>
                {platforms.map((p) => {
                  const info = play.availability(room, p, sheet.kind);
                  return (
                    <button key={p} type="button" role="radio" aria-checked={sheet.platform === p} className={`lite-option ${info.ok ? "" : "is-unavailable"}`} onClick={() => setSheet({ ...sheet, platform: p })}>
                      <span className="lite-option-mark" aria-hidden="true">{PLATFORM_MARK[p]}</span>
                      <span className="lite-option-text"><strong>{t(PLATFORM_LABEL[p])}</strong><small>{info.text}</small></span>
                      <span className="lite-option-radio" aria-hidden="true" />
                    </button>
                  );
                })}
              </div>
              {sheetPlatform && sheetPlatform !== "mister" && !play.pcOnline && (
                <p className="lite-muted">{t("Steam e Emulador abrem no seu PC: deixe o ShmupX Client aberto no PC com a mesma conta e escolha aqui.")}</p>
              )}
            </div>

            {error && <p className="lite-error" role="alert">{t(error)}</p>}
            <footer className="lite-sheet-footer">
              <button type="button" className="lite-primary" disabled={play.busy || !canPlay} onClick={() => void confirm()}>
                {play.busy ? t("Abrindo…") : sheetPlatform === "mister" && !player ? t("Ligar o MiSTer em Ajustes") : t("Jogar")}
              </button>
            </footer>
          </div>
        </div>
      )}
    </section>
  );
}
