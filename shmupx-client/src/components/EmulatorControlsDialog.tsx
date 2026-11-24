import { useI18n } from "../i18n";
import CabinetControlMapper from "./CabinetControlMapper";
import { useDialogFocus } from "./useDialogFocus";

/**
 * Controles do emulador numa janela no centro (clique no gabinete de Minhas coleções): o mesmo
 * mapeamento de Configurações › Controle. Fecha no ✕, no Esc ou clicando fora; Esc esperando um
 * aperto só cancela a espera.
 */
export default function EmulatorControlsDialog({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const ref = useDialogFocus(true, onClose);
  return (
    <div className="controller-dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={ref} role="dialog" aria-modal="true" aria-labelledby="emulator-controls-title" tabIndex={-1} className="controller-dialog emulator-controls-dialog">
        <header>
          <div>
            <h2 id="emulator-controls-title">{t("Controles do emulador")}</h2>
            <p>{t("Clique num botão do gabinete e aperte o botão do seu controle ou uma tecla. Vale para todos os jogos no ShmupX Emulator.")}</p>
          </div>
          <button type="button" className="secondary-action" onClick={onClose}>{t("Fechar")}</button>
        </header>
        <div className="controller-dialog-body"><CabinetControlMapper /></div>
      </div>
    </div>
  );
}
