import { useI18n } from "../i18n";
import { useWallet } from "../data/useWallet";

/**
 * Saldo de fichas (coins): cada partida valendo ranking gasta 1 (PC e MiSTer); o treino não gasta.
 * Sem moldura: a moeda e o número ficam à vista. A moeda tem frente e verso; cada vez que `spin`
 * muda (o Play Ranked), ela é lançada e gira no ar antes de cair de frente.
 */
export default function CreditsBadge({ uid, className = "", spin = 0, onPlay, playDisabled = false, playTitle, live = false }: {
  uid: string | undefined;
  className?: string;
  spin?: number;
  /** Com `onPlay`, a ficha é o botão do Play Ranked (insira a ficha para jogar valendo ranking). */
  onPlay?: () => void;
  playDisabled?: boolean;
  playTitle?: string;
  /** Play Ranked com live (só muda o nome do botão para leitores de tela). */
  live?: boolean;
}) {
  const { t } = useI18n();
  const credits = useWallet(uid);
  // A chave recomeça a animação a cada lançamento.
  const coin = (
    <span key={spin} className={`credits-coin ${spin > 0 ? "is-spinning" : ""}`} aria-hidden="true">
      <span className="credits-coin-3d">
        <img className="credits-coin-face" src="/img/shmupx-token-front.png" alt="" draggable={false} />
        <img className="credits-coin-face is-back" src="/img/shmupx-token-back.png" alt="" draggable={false} />
      </span>
    </span>
  );
  return (
    <div className={`credits-badge ${credits !== null && credits <= 5 ? "is-low" : ""} ${className}`} title={t("Fichas: cada partida valendo ranking gasta 1. O modo treino não gasta.")}>
      {onPlay
        ? <button type="button" className="credits-coin-button" data-tutorial="play-coin" onClick={onPlay} disabled={playDisabled} aria-label={t(live ? "Play Ranked com live" : "Play Ranked")}
            title={playTitle ?? t("Clique na ficha para jogar valendo ranking.")}>{coin}</button>
        : coin}
      <strong>{credits ?? "—"}</strong>
      <span className="credits-label">{t(credits === 1 ? "ficha" : "fichas")}</span>
    </div>
  );
}
