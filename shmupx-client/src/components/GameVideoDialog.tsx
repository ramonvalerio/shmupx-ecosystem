import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../i18n";
import { defaultEmulatorSettings, useEmulatorSettings } from "../data/emulatorSettings";
import { useDialogFocus } from "./useDialogFocus";
import DialogClose from "./DialogClose";
import GameVideoPanel from "./GameVideoPanel";

/**
 * Vídeo do jogo (clique na tela do gabinete em Minhas coleções e na sala): o painel de vídeo
 * (GameVideoPanel) com o gabinete do jogo, as mesmas opções de Configurações › Emulador, salvas na
 * hora. Fecha no X, no Esc ou clicando fora.
 */
export default function GameVideoDialog({ onClose, art, game, cabinet }: {
  onClose: () => void;
  art?: string;
  game?: string;
  /** O gabinete do jogo com a prévia na tela (quem abre a modal desenha o gabinete). */
  cabinet: (screen: ReactNode) => ReactNode;
}) {
  const { t } = useI18n();
  const ref = useDialogFocus(true, onClose);
  const { settings, update, saved } = useEmulatorSettings();

  return createPortal(
    <div className="controller-dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={ref} role="dialog" aria-modal="true" aria-labelledby="game-video-title" tabIndex={-1} className="arcade-monitor">
        <header className="am-head">
          <div>
            <h2 id="game-video-title">{t("Vídeo do jogo")}</h2>
            <p>{t("Como os jogos de arcade abrem neste computador. Vale a partir do próximo jogo aberto.")}</p>
          </div>
          <DialogClose onClick={onClose} />
        </header>

        <GameVideoPanel settings={settings} update={update} art={art} game={game} cabinet={cabinet} />

        <footer className="am-foot">
          <p role="status">{saved ? t("Salvo automaticamente neste computador.") : t("Preferências aplicadas nesta sessão. Não foi possível salvar neste dispositivo.")}</p>
          <button type="button" className="am-reset" onClick={() => update(defaultEmulatorSettings())}>{t("Restaurar padrão")}</button>
        </footer>
      </div>
    </div>,
    document.body,
  );
}
