import { useState } from "react";
import { useI18n } from "@client/i18n";
import { GAME_MODES } from "@client/data/gameModes";
import { useGameRanking, useGlobalRanking, type RankingMode } from "@client/data/useRanking";
import type { GameMode, GameRoom } from "@client/types/room";
import type { PlayPreset } from "./prefs";

const GLOBAL = "__global__";

/** Ranking na tela inicial: o Global (soma das notas dos 5 jogos mais jogados) ou o de um jogo, por modo. */
export default function RankingTab({ rooms, preset }: { rooms: GameRoom[]; preset: PlayPreset }) {
  const { t, locale } = useI18n();
  const [scope, setScope] = useState(GLOBAL);
  const [mode, setMode] = useState<GameMode>(preset.mode);
  const isGlobal = scope === GLOBAL;
  const global = useGlobalRanking(100);
  const game = useGameRanking(isGlobal ? "" : scope, mode as RankingMode);
  const current = isGlobal ? global : game;

  const rows = isGlobal
    ? (global.data?.entradas ?? []).map((e) => ({ id: e.jogador_id, position: e.posicao, name: e.nome, value: e.pontos.toFixed(2), detail: e.jogos_contados === 1 ? t("1 jogo") : t("{count} jogos", { count: e.jogos_contados }), pending: false }))
    : (game.data?.entradas ?? []).map((e) => ({ id: e.jogador_id, position: e.posicao, name: e.nome, value: e.score.toLocaleString(locale), detail: e.continues === 0 ? t("1CC") : (e.continues === 1 ? t("1 continue") : t("{count} continues", { count: e.continues })), pending: !e.verificada }));

  return (
    <div className="lite-content lite-ranking">
      <label className="settings-field">{t("Ranking")}
        <select value={scope} onChange={(event) => setScope(event.target.value)}>
          <option value={GLOBAL}>{t("Ranking Global")}</option>
          {rooms.map((room) => <option key={room.id} value={room.id}>{room.title}</option>)}
        </select>
      </label>
      {!isGlobal && (
        <div className="lite-preset-row" role="radiogroup" aria-label={t("Modo de jogo")}>
          {GAME_MODES.map((m) => <button key={m.id} type="button" role="radio" aria-checked={mode === m.id} onClick={() => setMode(m.id)}>{m.label}</button>)}
        </div>
      )}
      <p className="lite-muted">{isGlobal ? t("Soma das notas dos 5 jogos mais jogados de cada jogador.") : t("Melhor placar de cada jogador neste modo.")}</p>

      {current.status === "loading" && rows.length === 0 && <p className="lite-muted lite-center-text" role="status">{t("Carregando o ranking…")}</p>}
      {current.status === "error" && (
        <p className="lite-error" role="alert">{t("Não foi possível carregar o ranking agora.")} <button type="button" className="text-action" onClick={current.reload}>{t("Tentar de novo")}</button></p>
      )}
      {current.status === "ready" && rows.length === 0 && <p className="lite-muted lite-center-text">{t("Nenhum resultado ainda")}</p>}

      <ol className="lite-ranking-list">
        {rows.map((row) => (
          <li key={row.id} className={row.position <= 3 ? `is-top is-top-${row.position}` : ""}>
            <span className="lite-rank-position">{String(row.position).padStart(2, "0")}</span>
            <span className="lite-rank-name"><strong>{row.name}{row.pending && <span title={t("Resultado em revisão")}> *</span>}</strong><small>{row.detail}</small></span>
            <span className="lite-rank-value">{row.value}</span>
          </li>
        ))}
      </ol>
      {rows.some((row) => row.pending) && <p className="lite-note">{t("* Resultado em revisão.")}</p>}
    </div>
  );
}
