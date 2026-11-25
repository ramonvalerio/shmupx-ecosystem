import { useState } from "react";
import { useI18n } from "../i18n";
import { playingWhere, useAccountPlaying, type PlayingNow } from "../data/playing";
import { stopGame, useStoppingGame } from "../data/stopGame";

const KIND_LABEL: Record<PlayingNow["kind"], string> = { training: "Treino", ranked: "Ranked", live: "Live" };

/**
 * Indicador global do jogador, no centro do topo do Client: o que a conta está fazendo agora em
 * qualquer aparelho (gravando uma run aqui, jogando no PC, no MiSTer ou pelo celular) com o botão
 * PARAR JOGO (o mesmo da sala), ou "Disponível para jogar". A live no ar tem o próprio indicador (ClientShell).
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
  const busy = useStoppingGame();
  const [error, setError] = useState<string | null>(null);

  // STOP: o mesmo que o PARAR JOGO da sala (data/stopGame).
  async function stop() {
    setError(null);
    try {
      const practicePid = playing?.device === "pc" ? practiceGames[playing.gameId] : undefined;
      await stopGame({ uid, recording: Boolean(recording), practicePid });
    } catch (failure) {
      setError(typeof failure === "string" ? failure : failure instanceof Error ? failure.message : String(failure));
    }
  }

  const stopButton = (
    <button type="button" className="player-status-stop" disabled={busy} onClick={() => void stop()}
      title={t(recording ? "Encerrar a gravação da run" : "Parar o jogo e liberar a conta")}>
      <span aria-hidden="true" />{t(busy ? "Parando…" : "Parar jogo")}
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
