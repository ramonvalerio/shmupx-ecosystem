import { useI18n } from "../i18n";
import { useWallet } from "../data/useWallet";

/**
 * Saldo de fichas (coins): cada partida valendo ranking gasta 1 (PC e MiSTer); o treino não gasta.
 * Sem moldura: a moeda e o número ficam à vista. A moeda tem frente e verso; cada vez que `spin`
 * muda (o Play Ranked), ela é lançada e gira no ar antes de cair de frente.
 */
export default function CreditsBadge({ uid, className = "", spin = 0 }: { uid: string | undefined; className?: string; spin?: number }) {
  const { t } = useI18n();
  const credits = useWallet(uid);
  return (
    <div className={`credits-badge ${credits !== null && credits <= 5 ? "is-low" : ""} ${className}`} title={t("Fichas: cada partida valendo ranking gasta 1. O modo treino não gasta.")}>
      {/* A chave recomeça a animação a cada lançamento. */}
      <span key={spin} className={`credits-coin ${spin > 0 ? "is-spinning" : ""}`} aria-hidden="true">
        <span className="credits-coin-3d">
          <img className="credits-coin-face" src="/img/shmupx-token-front.png" alt="" draggable={false} />
          <img className="credits-coin-face is-back" src="/img/shmupx-token-back.png" alt="" draggable={false} />
        </span>
      </span>
      <strong>{credits ?? "—"}</strong>
      <span className="credits-label">{t(credits === 1 ? "ficha" : "fichas")}</span>
    </div>
  );
}
