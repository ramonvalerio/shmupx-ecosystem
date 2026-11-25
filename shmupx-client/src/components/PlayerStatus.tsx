import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import { misterAuth, readMisterPlayer } from "../data/misterPlayer";
import { readMisterLibrary } from "../data/misterLibrary";
import { clearPlaying, playingWhere, setClientMisterGame, useAccountPlaying, type PlayingNow } from "../data/playing";

const KIND_LABEL: Record<PlayingNow["kind"], string> = { training: "Training", ranked: "Ranked", live: "Live" };

/**
 * Indicador global do jogador, no centro do topo do Client: o que a conta está fazendo agora em
 * qualquer aparelho (gravando uma run aqui, jogando no PC, no MiSTer ou pelo celular) com o botão
 * STOP, ou "Disponível para jogar". A live no ar tem o próprio indicador (ClientShell).
 */
export default function PlayerStatus({ uid, recording, practiceGames }: {
  uid: string;
  /** Run sendo gravada neste PC. */
  recording: { title: string } | null;
  /** Jogos abertos em treino neste PC: id do jogo -> pid. */
  practiceGames: Record<string, number>;
}) {
  const { t } = useI18n();
  const playing = useAccountPlaying();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function stop() {
    setBusy(true);
    setError(null);
    try {
      if (recording) {
        // Encerra a gravação: a análise tira o placar e a run vai para o ranking.
        await invoke("finalizar_corrida_manual");
        return;
      }
      if (!playing) return;
      if (playing.platform === "mister") {
        // O MiSTer volta ao menu (o agente grava a partida e libera a conta); a conta é liberada
        // aqui também, mesmo que o MiSTer já esteja no menu ou desligado.
        const player = readMisterPlayer();
        const host = player?.host || readMisterLibrary()?.host;
        if (host) await invoke("mister_parar", { host, ...misterAuth(player?.password) }).catch(() => undefined);
        setClientMisterGame(null);
        await clearPlaying(uid);
        return;
      }
      const pid = practiceGames[playing.gameId];
      if (playing.device === "pc" && pid !== undefined) {
        await invoke("treino_parar", { pid });
        return;
      }
      // Aberto em outro aparelho (outro PC ou o celular): libera a conta daqui.
      await clearPlaying(uid);
    } catch (failure) {
      setError(typeof failure === "string" ? failure : failure instanceof Error ? failure.message : String(failure));
    } finally {
      setBusy(false);
    }
  }

  const stopButton = (
    <button type="button" className="player-status-stop" disabled={busy} onClick={() => void stop()}
      title={t(recording ? "Encerrar a gravação da run" : "Parar o jogo e liberar a conta")}>
      <span aria-hidden="true" />{t(busy ? "Parando…" : "STOP")}
    </button>
  );

  if (recording) {
    return (
      <div className="player-status is-recording" role="status">
        <span className="player-status-dot" aria-hidden="true" />
        <span className="player-status-text"><strong>{t("GRAVANDO RUN")}</strong><small>{recording.title}</small></span>
        {stopButton}
        {error && <small className="player-status-error" role="alert">{t(error)}</small>}
      </div>
    );
  }

  if (playing) {
    const where = playingWhere(playing);
    const training = playing.kind === "training";
    return (
      <div className={`player-status ${training ? "is-training" : "is-playing"}`} role="status">
        <span className="player-status-dot" aria-hidden="true" />
        <span className="player-status-text">
          <strong>{t(training ? "TREINANDO" : "JOGANDO")}</strong>
          <small title={`${playing.title} · ${t(where.platform)} · ${t(KIND_LABEL[playing.kind])} · ${t(where.device)}`}>
            {playing.title} · {t(where.platform)} · {t(KIND_LABEL[playing.kind])}
          </small>
        </span>
        {stopButton}
        {error && <small className="player-status-error" role="alert">{t(error)}</small>}
      </div>
    );
  }

  return (
    <div className="player-status is-free" role="status">
      <span className="player-status-dot" aria-hidden="true" />
      <span className="player-status-text"><strong>{t("DISPONÍVEL PARA JOGAR")}</strong></span>
    </div>
  );
}
