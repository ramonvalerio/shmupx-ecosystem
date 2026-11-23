import { useState } from "react";
import { useI18n } from "@client/i18n";
import { useGameRanking, type RankingMode } from "@client/data/useRanking";
import type { GameMode, GameRoom } from "@client/types/room";
import { modeFor, roomModes } from "./play";
import type { PlayPreset } from "./prefs";

/** Ranking da sala: o melhor placar de cada jogador neste jogo, por modo de jogo. */
export default function RankingTab({ room, preset }: { room: GameRoom; preset: PlayPreset }) {
  const { t, locale } = useI18n();
  const modes = roomModes(room);
  const [mode, setMode] = useState<GameMode>(() => modeFor(room, preset.mode));
  const ranking = useGameRanking(room.id, mode as RankingMode);
  const rows = (ranking.data?.entradas ?? []).map((e) => ({
    id: e.jogador_id, position: e.posicao, name: e.nome, value: e.score.toLocaleString(locale),
    detail: e.continues === 0 ? t("1CC") : e.continues === 1 ? t("1 continue") : t("{count} continues", { count: e.continues }),
    pending: !e.verificada,
  }));

  return (
    <section className="lite-content lite-ranking" aria-label={t("Ranking de {game}", { game: room.title })}>
      {modes.length > 1 && (
        <div className="lite-preset-row" role="radiogroup" aria-label={t("Modo de jogo")}>
          {modes.map((m) => <button key={m.id} type="button" role="radio" aria-checked={mode === m.id} onClick={() => setMode(m.id)}>{m.label}</button>)}
        </div>
      )}
      <p className="lite-muted">{t("Melhor placar de cada jogador neste modo.")}</p>

      {ranking.status === "loading" && rows.length === 0 && <p className="lite-muted lite-center-text" role="status">{t("Carregando o ranking…")}</p>}
      {ranking.status === "error" && (
        <p className="lite-error" role="alert">{t("Não foi possível carregar o ranking agora.")} <button type="button" className="text-action" onClick={ranking.reload}>{t("Tentar de novo")}</button></p>
      )}
      {ranking.status === "ready" && rows.length === 0 && <p className="lite-muted lite-center-text">{t("Nenhum resultado ainda")}</p>}

      <ol className="lite-ranking-list">
        {rows.map((row) => (
          <li key={row.id} className={row.position <= 3 ? "is-top" : ""}>
            <span className="lite-rank-position">{String(row.position).padStart(2, "0")}</span>
            <span className="lite-rank-name"><strong>{row.name}{row.pending && <span title={t("Resultado em revisão")}> *</span>}</strong><small>{row.detail}</small></span>
            <span className="lite-rank-value">{row.value}</span>
          </li>
        ))}
      </ol>
      {rows.some((row) => row.pending) && <p className="lite-note">{t("* Resultado em revisão.")}</p>}
    </section>
  );
}
