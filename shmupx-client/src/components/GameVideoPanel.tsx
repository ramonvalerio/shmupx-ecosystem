import { useEffect, useState, type ReactNode } from "react";
import { readMisterLibrary } from "../data/misterLibrary";
import { useI18n } from "../i18n";
import { EMULATOR_RESOLUTIONS, identifyMonitor, listMonitors, MISTER_MONITOR, SCREEN_MODE_CHOICES, screenMode, withScreenMode, type EmulatorResolution, type EmulatorSettings, type PcMonitor } from "../data/emulatorSettings";
import "../monitor.css";

/** Ícone do monitor CRT de arcade (15 kHz): um gabinete arcade. */
function ArcadeIcon() {
  return (
    <svg className="am-tile-svg" viewBox="3 0 34 30" aria-hidden="true">
      <path d="M12 1h16v6l-2 1v9l4 3v3h-2v6H12v-6h-2v-3l4-3V8l-2-1z" />
      <path d="M12 7h16M10 20h20" />
      <rect x="15.5" y="9.5" width="9" height="6" />
      <path d="M15 23h3M22 23h3" />
    </svg>
  );
}

/** Ícone do monitor CRT no MiSTer: o conector do cabo de rede (RJ45), por onde o vídeo vai. */
function NetworkIcon() {
  return (
    <svg className="am-tile-svg" viewBox="3 0 34 30" aria-hidden="true">
      <path d="M20 1v6" />
      <path d="M11 7h18v14H11z" />
      <path d="M14.5 21v3M17.5 21v3M20.5 21v3M23.5 21v3M26.5 21v3" />
      <path d="M16 7v-1h8v1M17 21v8h6v-8" />
    </svg>
  );
}

/**
 * Vídeo do jogo: como o ShmupX Emulator abre os jogos de arcade neste PC. À esquerda, o gabinete
 * arcade com a prévia na tela (em janela ou ocupando a tela, com as scanlines, animando a troca); à
 * direita, o modo de tela, o monitor dos jogos (os do PC e o monitor CRT no MiSTer) e a resolução
 * (só para monitor do PC). O mesmo na modal Vídeo do jogo (clique
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

  const mister = settings.resolution === "mister";
  const crt = settings.resolution === "crt15" || settings.resolution === "crt31" || mister;
  const fullscreen = !settings.windowed;
  const mode = SCREEN_MODE_CHOICES.find((m) => m.id === screenMode(settings)) ?? SCREEN_MODE_CHOICES[0];
  const chosen = monitors.find((m) => m.id === settings.monitor);
  const savedMonitorGone = settings.monitor !== "" && monitors.length > 0 && !chosen;
  const shown = chosen ?? monitors.find((m) => m.principal);
  const size = settings.resolution === "crt31" ? "480p · 31 kHz" : crt ? "240p · 15 kHz" : shown ? `${shown.largura}×${shown.altura}` : t("resolução do monitor");
  const where = mister ? "MiSTer" : shown ? t("Monitor {number}", { number: shown.numero }) : t("Monitor principal");
  // Monitor do PC escolhido: vindo do MiSTer, volta à resolução recomendada.
  const pickMonitor = (id: string) => update({ ...settings, monitor: id, resolution: mister ? "native" : settings.resolution });
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
            <div className="am-tiles is-three is-pair" role="radiogroup" aria-label={t("Modo de tela")}>
              {SCREEN_MODE_CHOICES.map((m) => (
                <button key={m.id} type="button" role="radio" aria-checked={mode.id === m.id} className="am-tile" onClick={() => update(withScreenMode(settings, m.id))}>
                  <span className={`am-tile-icon is-${m.id}`} aria-hidden="true" />
                  <span className="am-tile-name">{t(m.label)}</span>
                  <small>{t(m.detail)}</small>
                </button>
              ))}
            </div>
          </section>

          <section className="am-group" aria-label={t("Monitor dos jogos")}>
            <h3>{t("Monitor dos jogos")}</h3>
            <div className="am-tiles" role="radiogroup" aria-label={t("Monitor dos jogos")}>
              {monitors.map((m) => {
                const checked = !mister && (settings.monitor === m.id || (settings.monitor === "" && m.principal));
                return (
                  <button key={m.id} type="button" role="radio" aria-checked={checked} className="am-tile is-row"
                    onClick={() => { pickMonitor(m.principal ? "" : m.id); identifyMonitor(m.id); }}>
                    <span className="am-tile-number" aria-hidden="true">{m.numero}</span>
                    <span className="am-tile-name">{t("Monitor {number}", { number: m.numero })}</span>
                    <small>{m.largura}×{m.altura}{m.principal ? ` · ${t("principal do Windows")}` : ""}</small>
                  </button>
                );
              })}
              {/* Sem a lista do Windows (fora do app desktop): o principal. */}
              {monitors.length === 0 && (
                <button type="button" role="radio" aria-checked={!mister} className="am-tile is-row" onClick={() => pickMonitor("")}>
                  <span className="am-tile-icon is-lcd" aria-hidden="true" />
                  <span className="am-tile-name">{t("Monitor principal")}</span>
                  <small>{t("principal do Windows")}</small>
                </button>
              )}
              <button type="button" role="radio" aria-checked={mister} className="am-tile is-row" onClick={() => update({ ...settings, resolution: "mister" })}>
                <NetworkIcon />
                <span className="am-tile-name">{t(MISTER_MONITOR.label)}</span>
                <small>{t(MISTER_MONITOR.tile)}</small>
              </button>
            </div>
            {savedMonitorGone && !mister && <p className="am-note">{t("O monitor escolhido não está ligado agora: os jogos abrem no principal do Windows até ele voltar.")}</p>}
            {mister && <p className="am-note">{t(MISTER_MONITOR.detail)}</p>}
            {mister && !readMisterLibrary()?.host && <p className="am-note">{t("Conecte o seu MiSTer em Importar jogos › MiSTer FPGA para usar esta opção.")}</p>}
          </section>

          {/* A resolução é do monitor do PC; no MiSTer, o modo de 15 kHz do jogo vem do próprio MiSTer. */}
          {!mister && (
            <section className="am-group" aria-label={t("Resolução")}>
              <h3>{t("Resolução")}</h3>
              <div className="am-tiles" role="radiogroup" aria-label={t("Resolução")}>
                {EMULATOR_RESOLUTIONS.map((r) => (
                  <button key={r.id} type="button" role="radio" aria-checked={settings.resolution === r.id} className="am-tile is-row"
                    onClick={() => update({ ...settings, resolution: r.id as EmulatorResolution })}>
                    {r.id === "crt15" ? <ArcadeIcon /> : <span className={`am-tile-icon ${r.id === "native" ? "is-lcd" : "is-tube"}`} aria-hidden="true" />}
                    <span className="am-tile-name">{t(r.label)}</span>
                    <small>{t(r.tile)}</small>
                  </button>
                ))}
              </div>
              <p className="am-note">{t(resolution.detail)}</p>
            </section>
          )}
        </div>
      </div>
    </div>
  );
}
