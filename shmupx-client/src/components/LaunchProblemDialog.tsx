import { useState } from "react";
import { useI18n } from "../i18n";
import { chooseEmulatorFolder } from "../data/emulatorLibrary";
import type { MisterReach } from "../data/launchFailure";
import { useDialogFocus } from "./useDialogFocus";

/** O que impediu o jogo de abrir (ou de parar), do ponto de vista do jogador. */
export type LaunchProblemKind =
  | `mister-${MisterReach}`
  | "mister-not-in-menu"
  | "mister-failed"
  | "mister-stop-failed"
  | "steam-missing"
  | "steam-app-missing"
  | "steam-timeout"
  | "rom-missing"
  | "emulator-missing"
  | "launch-failed";

type Translate = ReturnType<typeof useI18n>["t"];

/** Textos para quem não entende de rede nem de emulador: o que houve e o que fazer, sem termos técnicos. */
function content(kind: LaunchProblemKind, game: string, t: Translate): { title: string; text: string; steps: string[] } {
  switch (kind) {
    case "mister-sem_resposta":
      return {
        title: t("Não conseguimos falar com o seu MiSTer"),
        text: t("O MiSTer não respondeu agora. Normalmente é algo simples de resolver:"),
        steps: [
          t("Confira se o MiSTer está ligado e mostrando o menu na TV."),
          t("Veja se ele está conectado à internet de casa (Wi-Fi ou cabo)."),
          t("Se acabou de ligar, espere uns 30 segundos e tente de novo."),
          t("Se continuar, desligue e ligue de novo o MiSTer e o aparelho da internet (roteador)."),
        ],
      };
    case "mister-outra_rede":
      return {
        title: t("O MiSTer está em outra rede"),
        text: t("Este computador e o MiSTer precisam estar conectados à mesma internet de casa."),
        steps: [
          t("Conecte este computador ao mesmo Wi-Fi em que o MiSTer está."),
          t("Se a sua internet tem mais de um nome de Wi-Fi (por exemplo, um com \"5G\" no nome ou um para visitantes), use o mesmo nos dois aparelhos."),
          t("Depois, tente de novo."),
        ],
      };
    case "mister-sem_rede":
      return {
        title: t("Este computador está sem internet"),
        text: t("Para abrir o jogo no MiSTer, este computador precisa estar conectado à internet de casa."),
        steps: [t("Conecte o computador ao Wi-Fi ou ao cabo de rede."), t("Depois, tente de novo.")],
      };
    case "mister-not-in-menu":
      return {
        title: t("{game} ainda não está no seu MiSTer", { game }),
        text: t("O MiSTer não tem este jogo na lista do ShmupX."),
        steps: [
          t("Confira se o jogo está no cartão de memória do MiSTer."),
          t("Em Importar jogos › MiSTer FPGA, conecte o MiSTer de novo para atualizar a lista."),
        ],
      };
    case "mister-failed":
      return {
        title: t("O jogo não abriu no MiSTer"),
        text: t("O MiSTer respondeu, mas não conseguiu abrir {game} agora.", { game }),
        steps: [t("Volte o MiSTer para o menu principal e tente de novo."), t("Se continuar, desligue e ligue o MiSTer.")],
      };
    case "mister-stop-failed":
      return {
        title: t("Não conseguimos parar o jogo no MiSTer"),
        text: t("O MiSTer não respondeu ao pedido para voltar ao menu."),
        steps: [
          t("Se o jogo ainda está aberto, volte ao menu pelo controle do MiSTer."),
          t("Confira se o MiSTer continua ligado e conectado à internet de casa."),
        ],
      };
    case "steam-missing":
      return {
        title: t("{game} não está instalado", { game }),
        text: t("O jogo não está instalado na Steam deste computador."),
        steps: [t("Abra a Steam e instale {game}.", { game }), t("Quando terminar, volte aqui e tente de novo.")],
      };
    case "steam-timeout":
      return {
        title: t("A Steam não abriu {game}", { game }),
        text: t("Pedimos o jogo à Steam, mas ele não abriu a tempo."),
        steps: [
          t("Abra a janela da Steam: ela pode estar pedindo uma atualização, para aceitar termos ou terminando de sincronizar."),
          t("Se o jogo abrir pela Steam, feche-o e tente de novo por aqui."),
          t("Se continuar, reinicie a Steam."),
        ],
      };
    case "steam-app-missing":
      return {
        title: t("Steam não encontrada"),
        text: t("Para jogar {game}, a Steam precisa estar instalada neste computador.", { game }),
        steps: [
          t("Instale a Steam (store.steampowered.com) e entre na sua conta."),
          t("Instale {game} pela Steam.", { game }),
          t("Depois, volte aqui e tente de novo."),
        ],
      };
    case "rom-missing":
      return {
        title: t("{game} não está nos seus ROMs", { game }),
        text: t("Para jogar no ShmupX Emulator, o ROM do jogo precisa estar na pasta de ROMs deste computador."),
        steps: [
          t("Coloque o ROM de {game} (arquivo .zip) na sua pasta de ROMs.", { game }),
          t("Se ele está em outra pasta, clique em “Selecionar pasta de ROMs” e indique onde."),
          t("Volte aqui e tente de novo."),
        ],
      };
    case "emulator-missing":
      return {
        title: t("ShmupX Emulator não encontrado"),
        text: t("O ShmupX Emulator vem com o ShmupX Client, mas não foi encontrado nesta instalação."),
        steps: [t("Reinstale o ShmupX Client."), t("Depois, volte aqui e tente de novo.")],
      };
    case "launch-failed":
      return {
        title: t("O jogo não abriu"),
        text: t("Algo impediu {game} de abrir agora.", { game }),
        steps: [t("Se o jogo abriu pela metade, feche-o e tente de novo."), t("Se continuar, reinicie o computador.")],
      };
  }
}

/** Aviso centralizado quando o jogo não abre: o que houve, o que fazer e "Tentar de novo". */
export function LaunchProblemDialog({ kind, game, onRetry, onClose }: { kind: LaunchProblemKind; game: string; onRetry?: () => void; onClose: () => void }) {
  const { t } = useI18n();
  const ref = useDialogFocus(true, onClose);
  const { title, text, steps } = content(kind, game, t);
  // ROMs fora das pastas de costume: o jogador indica a pasta aqui mesmo e tenta de novo.
  const [picking, setPicking] = useState(false);
  const [folderError, setFolderError] = useState<string | null>(null);
  async function pickFolder() {
    setPicking(true);
    setFolderError(null);
    try {
      const result = await chooseEmulatorFolder(t("Pasta de ROMs"));
      if (!result) return;
      if (!result.found) {
        setFolderError(t("Não encontramos ROMs neste computador."));
        return;
      }
      onClose();
      onRetry?.();
    } catch (error) {
      setFolderError(t(error instanceof Error ? error.message : String(error)));
    } finally {
      setPicking(false);
    }
  }
  return (
    <div className="controller-dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={ref} role="alertdialog" aria-modal="true" aria-labelledby="launch-problem-title" aria-describedby="launch-problem-text" tabIndex={-1} className="controller-dialog close-room-dialog launch-problem-dialog">
        <header>
          <div>
            <h2 id="launch-problem-title">{title}</h2>
            <p id="launch-problem-text">{text}</p>
          </div>
        </header>
        {steps.length > 0 && <ol className="launch-problem-steps">{steps.map((step) => <li key={step}>{step}</li>)}</ol>}
        {kind === "rom-missing" && <div className="launch-problem-folder">
          <button type="button" className="secondary-action" disabled={picking} onClick={() => void pickFolder()}>{t(picking ? "Abrindo…" : "Selecionar pasta de ROMs")}</button>
          {folderError && <p className="mode-error-inline" role="alert">{folderError}</p>}
        </div>}
        <footer>
          {onRetry ? <button type="button" className="secondary-action" onClick={onClose}>{t("Fechar")}</button> : <span />}
          <button type="button" className="primary-action" onClick={() => { onClose(); onRetry?.(); }}>{t(onRetry ? "Tentar de novo" : "Entendi")}</button>
        </footer>
      </div>
    </div>
  );
}
