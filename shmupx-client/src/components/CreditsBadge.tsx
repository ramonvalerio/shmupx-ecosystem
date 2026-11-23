import { useI18n } from "../i18n";
import { useWallet } from "../data/useWallet";

/**
 * Saldo de fichas (coins): cada partida valendo ranking gasta 1 (PC e MiSTer); o treino não gasta.
 * Sem moldura: a moeda e o número ficam à vista.
 */
export default function CreditsBadge({ uid, className = "" }: { uid: string | undefined; className?: string }) {
  const { t } = useI18n();
  const credits = useWallet(uid);
  return (
    <div className={`credits-badge ${credits !== null && credits <= 5 ? "is-low" : ""} ${className}`} title={t("Fichas: cada partida valendo ranking gasta 1. O modo treino não gasta.")}>
      <img className="credits-coin" src="/img/coin.png" alt="" aria-hidden="true" />
      <strong>{credits ?? "—"}</strong>
      <span>{t(credits === 1 ? "ficha" : "fichas")}</span>
    </div>
  );
}
