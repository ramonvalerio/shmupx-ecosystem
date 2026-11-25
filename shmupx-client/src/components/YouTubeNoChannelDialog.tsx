import { createPortal } from "react-dom";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import { useDialogFocus } from "./useDialogFocus";

const CREATE_CHANNEL = "https://www.youtube.com/create_channel";
const LIVE_FEATURES = "https://www.youtube.com/features";

function openYouTube(url: string) {
  if (isTauri()) void invoke("live_abrir", { url });
  else window.open(url, "_blank", "noopener");
}

/**
 * A conta Google escolhida ao conectar o YouTube não tem canal: passo a passo para criar o canal
 * (na mesma conta), liberar as lives e conectar de novo. Abre por cima das Configurações.
 */
export default function YouTubeNoChannelDialog({ busy, onRetry, onClose }: { busy: boolean; onRetry: () => void; onClose: () => void }) {
  const { t } = useI18n();
  const dialogRef = useDialogFocus(true, onClose);
  return createPortal(
    <div className="controller-dialog-backdrop youtube-help-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby="youtube-help-title" tabIndex={-1} className="controller-dialog youtube-help">
        <header>
          <div>
            <h2 id="youtube-help-title">{t("Sua conta Google ainda não tem canal no YouTube")}</h2>
            <p>{t("Para transmitir ao vivo, o ShmupX usa um canal do YouTube da conta Google que você escolheu. Criar é grátis e leva um minuto.")}</p>
          </div>
        </header>
        <div className="controller-dialog-body">
          <ol className="youtube-help-steps">
            <li>
              <strong>{t("Crie o canal")}</strong>
              <span>{t("Abra o YouTube com a mesma conta Google, confirme o nome e a foto e clique em \"Criar canal\".")}</span>
              <button type="button" className="primary-action" onClick={() => openYouTube(CREATE_CHANNEL)}>{t("Criar meu canal no YouTube")}</button>
            </li>
            <li>
              <strong>{t("Libere as transmissões ao vivo")}</strong>
              <span>{t("Em Recursos do YouTube, ative \"Transmissões ao vivo\". Na primeira vez, o YouTube pode levar até 24 horas para liberar.")}</span>
              <button type="button" className="secondary-action" onClick={() => openYouTube(LIVE_FEATURES)}>{t("Abrir os recursos do canal")}</button>
            </li>
            <li>
              <strong>{t("Conecte de novo")}</strong>
              <span>{t("Volte aqui e conecte, escolhendo a mesma conta Google.")}</span>
            </li>
          </ol>
          <p className="muted">{t("Se o navegador abrir o YouTube em outra conta Google, troque de conta pela foto no canto superior direito antes de criar o canal.")}</p>
        </div>
        <footer className="youtube-help-actions">
          <button type="button" className="secondary-action" onClick={onClose}>{t("Fechar")}</button>
          <button type="button" className="primary-action" disabled={busy} onClick={onRetry}>{busy ? t("Aguardando o Google…") : t("Já criei, conectar de novo")}</button>
        </footer>
      </div>
    </div>,
    // Dentro do .client-shell, onde estão as cores do tema; no body o fundo ficava transparente.
    document.querySelector(".client-shell") ?? document.body,
  );
}
