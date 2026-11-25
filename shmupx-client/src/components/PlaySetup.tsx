import { useId } from "react";
import { useI18n } from "../i18n";
import type { PlayOn } from "../types/room";
import type { useRoomPlay } from "../data/roomPlay";

/**
 * Modo de jogo e plataforma (os mesmos na sala e no painel da Coleção). A plataforma em que o jogo
 * não está sincronizado fica desabilitada, com a dica de importar.
 */
export default function PlaySetup({ play, owns, disabled, compact = false }: {
  play: ReturnType<typeof useRoomPlay>;
  owns: (platform: PlayOn) => boolean;
  disabled: boolean;
  compact?: boolean;
}) {
  const { t } = useI18n();
  const uid = useId();
  return (
    <div className={`room-play-setup ${compact ? "is-compact" : ""}`}>
      {play.modes.length > 0 && (
        <div className="room-setup-group">
          <span id={`${uid}-mode`}>{t("Modo de jogo")}</span>
          <div className="room-chips" role="radiogroup" aria-labelledby={`${uid}-mode`} data-tutorial="play-modes">
            {play.modes.map((m) => (
              <button key={m.id} type="button" role="radio" aria-checked={play.mode === m.id} disabled={disabled || m.soon} title={m.soon ? t("Em breve") : t(m.detail)} onClick={() => play.choose({ mode: m.id })}>
                {m.label}{m.soon && <small className="room-chip-soon">{t("Em breve")}</small>}
              </button>
            ))}
          </div>
        </div>
      )}
      {play.options.length > 0 && (
        <div className="room-setup-group">
          <span id={`${uid}-platform`}>{t("Plataforma")}</span>
          <div className="room-chips" role="radiogroup" aria-labelledby={`${uid}-platform`}>
            {play.options.map((p) => {
              const owned = owns(p.id);
              return (
                <button key={p.id} type="button" role="radio" aria-checked={play.platform === p.id} disabled={disabled || !owned} onClick={() => play.choose({ platform: p.id })}
                  title={owned ? undefined : t("Este jogo não está sincronizado em {platform}: importe a biblioteca em Importar jogos.", { platform: t(p.label) })}>
                  {t(p.label)}
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
