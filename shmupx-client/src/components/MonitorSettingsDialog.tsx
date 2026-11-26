import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../i18n";
import { defaultEmulatorSettings, EMULATOR_RESOLUTIONS, listMonitors, useEmulatorSettings, type EmulatorResolution, type PcMonitor } from "../data/emulatorSettings";
import { useDialogFocus } from "./useDialogFocus";
import "../monitor.css";

/**
 * Monitor dos jogos (clique na tela do gabinete em Minhas coleções e na sala), com cara de monitor de
 * arcade: à esquerda, um tubo de CRT com a prévia de como o jogo abre (em janela ou ocupando a tela,
 * com as scanlines); à direita, o painel de serviço com o modo de tela, o monitor e a resolução. As
 * mesmas opções de Configurações › Emulador, salvas na hora. Fecha no botão, no Esc ou clicando fora.
 */
export default function MonitorSettingsDialog({ onClose, art, game }: { onClose: () => void; art?: string; game?: string }) {
  const { t } = useI18n();
  const ref = useDialogFocus(true, onClose);
  const { settings, update, saved } = useEmulatorSettings();
  const [monitors, setMonitors] = useState<PcMonitor[]>([]);
  useEffect(() => { listMonitors().then(setMonitors).catch(() => setMonitors([])); }, []);

  const crt = settings.resolution === "crt15";
  const fullscreen = !settings.windowed;
  const chosen = monitors.find((m) => m.id === settings.monitor);
  const savedMonitorGone = settings.monitor !== "" && monitors.length > 0 && !chosen;
  const shown = chosen ?? monitors.find((m) => m.principal);
  const size = crt ? "240p · 15 kHz" : shown ? `${shown.largura}×${shown.altura}` : t("resolução do monitor");
  const where = shown ? t("Monitor {number}", { number: shown.numero }) : t("Monitor principal");
  const resolution = EMULATOR_RESOLUTIONS.find((r) => r.id === settings.resolution) ?? EMULATOR_RESOLUTIONS[0];

  return createPortal(
    <div className="controller-dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={ref} role="dialog" aria-modal="true" aria-labelledby="monitor-settings-title" tabIndex={-1} className="arcade-monitor">
        <span className="am-corner is-tl" aria-hidden="true" /><span className="am-corner is-tr" aria-hidden="true" />
        <span className="am-corner is-bl" aria-hidden="true" /><span className="am-corner is-br" aria-hidden="true" />
        <header className="am-head">
          <div>
            <span className="am-kicker">{t("ShmupX Emulator")}</span>
            <h2 id="monitor-settings-title">{t("Monitor")}</h2>
            <p>{t("Como os jogos de arcade abrem neste computador. Vale a partir do próximo jogo aberto.")}</p>
          </div>
          <button type="button" className="am-close" onClick={onClose} aria-label={t("Fechar")}>✕</button>
        </header>

        <div className="am-body">
          {/* O tubo: prévia de como o jogo abre. */}
          <figure className="am-crt" aria-label={t("Prévia: {mode} no {monitor}", { mode: fullscreen ? t("tela cheia") : t("janela"), monitor: where })}>
            <div className="am-bezel">
              <div className={`am-screen${fullscreen ? " is-full" : ""}${crt ? " is-crt" : ""}`}>
                <div className="am-game">
                  {!fullscreen && <span className="am-game-bar" aria-hidden="true"><i /><i /><i /></span>}
                  <div className="am-game-image" style={art ? { backgroundImage: `url("${art}")` } : undefined}>
                    {!art && <span>{game ?? "SHMUPX"}</span>}
                  </div>
                </div>
                <span className="am-scanlines" aria-hidden="true" />
                <span className="am-glare" aria-hidden="true" />
              </div>
            </div>
            <figcaption className="am-plate">
              <span className="am-led" aria-hidden="true" />
              <span>{where}</span><span>{size}</span><span>{fullscreen ? t("Tela cheia") : t("Janela")}</span>
            </figcaption>
          </figure>

          {/* O painel de serviço. */}
          <div className="am-panel">
            <section className="am-group" aria-labelledby="am-mode">
              <h3 id="am-mode">{t("Modo de tela")}</h3>
              <div className="am-tiles is-two" role="radiogroup" aria-labelledby="am-mode">
                <button type="button" role="radio" aria-checked={!fullscreen} className="am-tile" onClick={() => update({ ...settings, windowed: true, scanlines: true })}>
                  <span className="am-tile-icon is-window" aria-hidden="true" />
                  <span className="am-tile-name">{t("Janela")}</span>
                  <small>{t("Ao lado do Client e do chat")}</small>
                </button>
                <button type="button" role="radio" aria-checked={fullscreen} className="am-tile" onClick={() => update({ ...settings, windowed: false, scanlines: true })}>
                  <span className="am-tile-icon is-full" aria-hidden="true" />
                  <span className="am-tile-name">{t("Tela cheia")}</span>
                  <small>{t("O jogo ocupa o monitor inteiro")}</small>
                </button>
              </div>
            </section>

            {(monitors.length > 1 || savedMonitorGone) && (
              <section className="am-group" aria-labelledby="am-where">
                <h3 id="am-where">{t("Monitor dos jogos")}</h3>
                <div className="am-tiles" role="radiogroup" aria-labelledby="am-where">
                  {monitors.map((m) => {
                    const checked = settings.monitor === m.id || (settings.monitor === "" && m.principal);
                    return (
                      <button key={m.id} type="button" role="radio" aria-checked={checked} className="am-tile is-row"
                        onClick={() => update({ ...settings, monitor: m.principal ? "" : m.id })}>
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

            <section className="am-group" aria-labelledby="am-resolution">
              <h3 id="am-resolution">{t("Resolução")}</h3>
              <div className="am-tiles" role="radiogroup" aria-labelledby="am-resolution">
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

        <footer className="am-foot">
          <p role="status">{saved ? t("Salvo automaticamente neste computador.") : t("Preferências aplicadas nesta sessão. Não foi possível salvar neste dispositivo.")}</p>
          <button type="button" className="am-reset" onClick={() => update(defaultEmulatorSettings())}>{t("Restaurar padrão")}</button>
        </footer>
      </div>
    </div>,
    document.body,
  );
}
