import { createPortal } from "react-dom";
import { useI18n } from "../i18n";
import { useDialogFocus } from "./useDialogFocus";
import { TUTORIAL_TOPICS, type TutorialTopic } from "./FirstRunTutorial";

/** HOW TO USE: o jogador escolhe a dúvida e o tutorial mostra só aquele caminho. */
export default function HowToUseDialog({ onPick, onClose }: { onPick: (topic: TutorialTopic) => void; onClose: () => void }) {
  const { t } = useI18n();
  const dialogRef = useDialogFocus(true, onClose);
  return createPortal(
    <div className="controller-dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby="how-to-use-title" tabIndex={-1} className="controller-dialog how-to-use-dialog">
        <header>
          <div>
            <h2 id="how-to-use-title">{t("Qual é a sua dúvida?")}</h2>
            <p>{t("Escolha um assunto e o ShmupX mostra, passo a passo, onde tocar.")}</p>
          </div>
        </header>
        <div className="controller-dialog-body">
          <div className="import-source-list how-to-use-list">
            {TUTORIAL_TOPICS.map((topic, index) => (
              <button key={topic.id} type="button" className="import-source-option" onClick={() => onPick(topic.id)}>
                <span className="how-to-use-number" aria-hidden="true">{index + 1}</span>
                <span><strong>{t(topic.title)}</strong><small>{t(topic.detail)}</small></span>
              </button>
            ))}
          </div>
        </div>
        <footer>
          <button type="button" className="secondary-action" onClick={onClose}>{t("Fechar")}</button>
        </footer>
      </div>
    </div>,
    document.body,
  );
}
