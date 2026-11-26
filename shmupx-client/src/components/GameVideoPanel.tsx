import { useEffect, useState, type ReactNode } from "react";
import { useI18n } from "../i18n";
import { EMULATOR_RESOLUTIONS, identifyMonitor, listMonitors, SCREEN_MODES, screenMode, withScreenMode, type EmulatorResolution, type EmulatorSettings, type PcMonitor } from "../data/emulatorSettings";
import "../monitor.css";

/**
 * Vídeo do jogo: como o ShmupX Emulator abre os jogos de arcade neste PC. À esquerda, o gabinete
 * arcade com a prévia na tela (em janela ou ocupando a tela, com as scanlines, animando a troca); à
 * direita, o modo de tela, o monitor dos jogos e a resolução. O mesmo na modal Vídeo do jogo (clique
 * na tela do gabinete) e em Configurações › Emulador. Sem `cabinet`, a prévia fica numa tela solta.
 */
export default function GameVideoPanel({ settings, update, art, game, cabinet }: {
  settings: EmulatorSettings;
  update: (next: EmulatorSettings) => void;
  /** Capa do jogo na prévia. */
  art?: string;
  game?: string;
  /** O gabinete do jogo com a prévia na tela. */
  cabinet?: (screen: ReactNode) => ReactNode;
}) {
  const { t } = useI18n();
  const [monitors, setMonitors] = useState<PcMonitor[]>([]);
  useEffect(() => { listMonitors().then(setMonitors).catch(() => setMonitors([])); }, []);

  const crt = settings.resolution === "crt15";
  const fullscreen = !settings.windowed;
  const mode = SCREEN_MODES.find((m) => m.id === screenMode(settings)) ?? SCREEN_MODES[0];
  const chosen = monitors.find((m) => m.id === settings.monitor);
  const savedMonitorGone = settings.monitor !== "" && monitors.length > 0 && !chosen;
  const shown = chosen ?? monitors.find((m) => m.principal);
  const size = crt ? "240p · 15 kHz" : shown ? `${shown.largura}×${shown.altura}` : t("resolução do monitor");
  const where = shown ? t("Monitor {number}", { number: shown.numero }) : t("Monitor principal");
  const resolution = EMULATOR_RESOLUTIONS.find((r) => r.id === settings.resolution) ?? EMULATOR_RESOLUTIONS[0];

  const screen = (
    <div className={`am-screen${fullscreen ? " is-full" : ""}${crt ? " is-crt" : ""}`}>
      <div className="am-game">
        <span className="am-game-bar" aria-hidden="true"><i /><i /><i /></span>
        <div className="am-game-image" style={art ? { backgroundImage: `url("${art}")` } : undefined}>
          {!art && <span>{game ?? "SHMUPX"}</span>}
        </div>
      </div>
      <span className="am-scanlines" aria-hidden="true" />
      {/* Ao trocar o modo ou a resolução, a tela "troca de vídeo" (o key reinicia a animação). */}
      <span key={`${mode.id}-${settings.resolution}`} className="am-switch" aria-hidden="true" />
    </div>
  );

  return (
    <div className="game-video">
      <div className="am-body">
        {/* O gabinete: na tela dele, a prévia de como o jogo abre. */}
        <figure className="am-crt" aria-label={t("Prévia: {mode} no {monitor}", { mode: t(mode.label).toLowerCase(), monitor: where })}>
          {cabinet ? cabinet(screen) : <div className="am-screen-frame">{screen}</div>}
          <figcaption className="am-plate">
            <span className="am-led" aria-hidden="true" />
            <span>{where}</span><span>{size}</span><span>{t(mode.label)}</span>
          </figcaption>
        </figure>

        {/* O painel de serviço. */}
        <div className="am-panel">
          <section className="am-group" aria-label={t("Modo de tela")}>
            <h3>{t("Modo de tela")}</h3>
            <div className="am-tiles is-three" role="radiogroup" aria-label={t("Modo de tela")}>
              {SCREEN_MODES.map((m) => (
                <button key={m.id} type="button" role="radio" aria-checked={mode.id === m.id} className="am-tile" onClick={() => update(withScreenMode(settings, m.id))}>
                  <span className={`am-tile-icon is-${m.id}`} aria-hidden="true" />
                  <span className="am-tile-name">{t(m.label)}</span>
                  <small>{t(m.detail)}</small>
                </button>
              ))}
            </div>
          </section>

          {(monitors.length > 1 || savedMonitorGone) && (
            <section className="am-group" aria-label={t("Monitor dos jogos")}>
              <h3>{t("Monitor dos jogos")}</h3>
              <div className="am-tiles" role="radiogroup" aria-label={t("Monitor dos jogos")}>
                {monitors.map((m) => {
                  const checked = settings.monitor === m.id || (settings.monitor === "" && m.principal);
                  return (
                    <button key={m.id} type="button" role="radio" aria-checked={checked} className="am-tile is-row"
                      onClick={() => { update({ ...settings, monitor: m.principal ? "" : m.id }); identifyMonitor(m.id); }}>
                      <span className="am-tile-number" aria-hidden="true">{m.numero}</span>
                      <span className="am-tile-name">{t("Monitor {number}", { number: m.numero })}</span>
                      <small>{m.largura}×{m.altura}{m.principal ? ` · ${t("principal do Windows")}` : ""}</small>
                    </button>
                  );
                })}
              </div>
              {savedMonitorGone && <p className="am-note">{t("O monitor escolhido não está ligado agora: os jogos abrem no principal do Windows até ele voltar.")}</p>}
            </section>
          )}

          <section className="am-group" aria-label={t("Resolução")}>
            <h3>{t("Resolução")}</h3>
            <div className="am-tiles" role="radiogroup" aria-label={t("Resolução")}>
              {EMULATOR_RESOLUTIONS.map((r) => (
                <button key={r.id} type="button" role="radio" aria-checked={settings.resolution === r.id} className="am-tile is-row"
                  onClick={() => update({ ...settings, resolution: r.id as EmulatorResolution })}>
                  <span className={`am-tile-icon ${r.id === "crt15" ? "is-tube" : "is-lcd"}`} aria-hidden="true" />
                  <span className="am-tile-name">{r.id === "crt15" ? t("TV de tubo 15 kHz") : t("Monitor do PC")}</span>
                  <small>{r.id === "crt15" ? t("240p, para monitor de arcade") : t("Recomendado")}</small>
                </button>
              ))}
            </div>
            <p className="am-note">{t(resolution.detail)}</p>
          </section>
        </div>
      </div>
    </div>
  );
}
