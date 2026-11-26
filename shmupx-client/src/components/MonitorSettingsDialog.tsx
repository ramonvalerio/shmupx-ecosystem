import { createPortal } from "react-dom";
import { useI18n } from "../i18n";
import { defaultEmulatorSettings, useEmulatorSettings } from "../data/emulatorSettings";
import { EmulatorVideoFields } from "./EmulatorVideoSettings";
import { useDialogFocus } from "./useDialogFocus";

/**
 * Monitor dos jogos numa janela no centro (clique na tela do gabinete em Minhas coleções e na sala):
 * as mesmas opções de Configurações › Emulador. Fecha no botão, no Esc ou clicando fora.
 */
export default function MonitorSettingsDialog({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const ref = useDialogFocus(true, onClose);
  const { settings, update, saved } = useEmulatorSettings();
  return createPortal(
    <div className="controller-dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={ref} role="dialog" aria-modal="true" aria-labelledby="monitor-settings-title" tabIndex={-1} className="controller-dialog monitor-settings-dialog">
        <header>
          <div>
            <h2 id="monitor-settings-title">{t("Monitor")}</h2>
            <p>{t("Como o ShmupX Emulator abre os jogos de arcade neste computador. Vale a partir do próximo jogo aberto.")}</p>
          </div>
          <button type="button" className="secondary-action" onClick={onClose}>{t("Fechar")}</button>
        </header>
        <div className="controller-dialog-body monitor-settings-body">
          <EmulatorVideoFields settings={settings} update={update} />
          <div className="monitor-settings-footer">
            <p className="muted" role="status">{saved ? t("As alterações são salvas automaticamente neste computador.") : t("Preferências aplicadas nesta sessão. Não foi possível salvar neste dispositivo.")}</p>
            <button type="button" className="secondary-action" onClick={() => update(defaultEmulatorSettings())}>{t("Restaurar padrão do emulador")}</button>
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
}
