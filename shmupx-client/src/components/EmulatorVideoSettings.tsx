import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import { EMULATOR_RESOLUTIONS, listMonitors, type EmulatorResolution, type EmulatorSettings, type PcMonitor } from "../data/emulatorSettings";

/**
 * Como o ShmupX Emulator mostra o jogo neste PC: tela cheia, monitor dos jogos e resolução. Usado
 * em Configurações › Emulador e na janela do monitor (clique na tela do gabinete).
 */
export function EmulatorVideoFields({ settings, update }: { settings: EmulatorSettings; update: (next: EmulatorSettings) => void }) {
  const { t } = useI18n();
  const resolution = EMULATOR_RESOLUTIONS.find((r) => r.id === settings.resolution) ?? EMULATOR_RESOLUTIONS[0];
  const [monitors, setMonitors] = useState<PcMonitor[]>([]);
  useEffect(() => { listMonitors().then(setMonitors).catch(() => setMonitors([])); }, []);
  const monitorLabel = (m: PcMonitor) => t(m.principal ? "Monitor {number} · {width}×{height} (principal do Windows)" : "Monitor {number} · {width}×{height}", { number: m.numero, width: m.largura, height: m.altura });
  const savedMonitorGone = settings.monitor !== "" && monitors.length > 0 && !monitors.some((m) => m.id === settings.monitor);
  return <>
    <label className="checkbox-field"><input type="checkbox" checked={!settings.windowed} onChange={(event) => update({ ...settings, windowed: !event.target.checked, scanlines: true })} />{t("Abrir sempre em tela cheia")}</label>
    {(monitors.length > 1 || savedMonitorGone) && <label className="settings-field">{t("Monitor dos jogos")}<select value={settings.monitor} onChange={(event) => update({ ...settings, monitor: event.target.value })}>
      <option value="">{t("O principal do Windows")}</option>
      {monitors.map((m) => <option key={m.id} value={m.id}>{monitorLabel(m)}</option>)}
      {savedMonitorGone && <option value={settings.monitor}>{t("Monitor escolhido (desligado agora)")}</option>}
    </select></label>}
    {savedMonitorGone && <p className="muted">{t("O monitor escolhido não está ligado agora: os jogos abrem no principal do Windows até ele voltar.")}</p>}
    <label className="settings-field">{t("Resolução de vídeo")}<select value={settings.resolution} onChange={(event) => update({ ...settings, resolution: event.target.value as EmulatorResolution })}>{EMULATOR_RESOLUTIONS.map((r) => <option key={r.id} value={r.id}>{t(r.label)}</option>)}</select></label>
    <p className="muted">{t(resolution.detail)}</p>
  </>;
}
