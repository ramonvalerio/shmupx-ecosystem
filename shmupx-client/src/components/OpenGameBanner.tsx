import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import { misterAuth, readMisterPlayer } from "../data/misterPlayer";
import { clearPlaying, playingWhere, useAccountPlaying } from "../data/playing";

const KIND_LABEL = { training: "modo treino", ranked: "modo ranqueado", live: "ao vivo" } as const;

/**
 * Jogo aberto na conta por outro aparelho (o celular, no MiSTer): um jogo por vez, então o Client
 * mostra qual é e deixa parar daqui (o MiSTer volta ao menu e a conta fica livre).
 */
export default function OpenGameBanner({ uid }: { uid: string }) {
  const { t } = useI18n();
  const open = useAccountPlaying();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!open || open.device !== "lite") return null;
  const where = playingWhere(open);

  async function stop() {
    if (!open) return;
    setBusy(true);
    setError(null);
    try {
      const player = readMisterPlayer();
      if (open.platform === "mister" && player) {
        // Mesmo que o MiSTer já esteja no menu (ou desligado), a conta é liberada em seguida.
        await invoke("mister_parar", { host: player.host, ...misterAuth(player.password) }).catch(() => undefined);
      }
      await clearPlaying(uid);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="open-game-banner" role="status">
      <span className="presence-dot is-live" aria-hidden="true" />
      <span>
        <strong>{t("Jogo aberto pelo celular: {game}", { game: open.title })}</strong>
        <small>{t(where.platform)} · {t(KIND_LABEL[open.kind])} · {t("Pare este jogo para abrir outro.")}</small>
      </span>
      {error && <small className="open-game-error">{t(error)}</small>}
      <button type="button" className="room-action stop" disabled={busy} onClick={() => void stop()}>{t(busy ? "Encerrando o jogo…" : "Parar jogo")}</button>
    </div>
  );
}
