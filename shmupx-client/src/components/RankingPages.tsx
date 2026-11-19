import { useState } from "react";
import { useI18n } from "../i18n";
import { useGameRanking, useGlobalRanking } from "../data/useRanking";
import type { GameRoom } from "../types/room";
import { PageHeading } from "./ClientPages";

const GLOBAL = "__global__";

function StateMessage({ status, empty, onRetry }: { status: "loading" | "ready" | "error"; empty: boolean; onRetry: () => void }) {
  const { t } = useI18n();
  if (status === "loading" && empty) return <p className="empty-state" role="status">{t("Carregando o ranking…")}</p>;
  if (status === "error") return <p className="empty-state" role="alert">{t("Não foi possível carregar o ranking agora.")} <button type="button" className="text-action" onClick={onRetry}>{t("Tentar de novo")}</button></p>;
  if (status === "ready" && empty) return <p className="empty-state">{t("Nenhum resultado ainda")}</p>;
  return null;
}

/** Top 3 do Ranking Global para o painel do início. */
export function GlobalRankingPreview({ onMore }: { onMore: () => void }) {
  const { t } = useI18n();
  const ranking = useGlobalRanking(3);
  const entries = ranking.data?.entradas ?? [];
  return (
    <>
      <StateMessage status={ranking.status} empty={entries.length === 0} onRetry={ranking.reload} />
      {entries.map((entry) => (
        <div className="dashboard-row" key={entry.jogador_id}>
          <span className="row-position">{entry.posicao}</span>
          <strong>{entry.nome}</strong>
          <span className="score-value">{entry.pontos.toFixed(2)}</span>
        </div>
      ))}
      <button className="text-action" onClick={onMore}>{t("Consultar ranking →")}</button>
    </>
  );
}

/** Ranking Global (soma das notas dos 5 jogos mais jogados) ou o ranking de um jogo. */
export function RankingPage({ rooms }: { rooms: GameRoom[] }) {
  const { t, locale } = useI18n();
  const [scope, setScope] = useState(GLOBAL);
  const [query, setQuery] = useState("");
  const [selectedPlayer, setSelectedPlayer] = useState<string | null>(null);
  const isGlobal = scope === GLOBAL;
  const global = useGlobalRanking(100);
  const game = useGameRanking(isGlobal ? "" : scope);
  const term = query.trim().toLowerCase();
  const room = rooms.find((item) => item.id === scope);

  const globalEntries = global.data?.entradas ?? [];
  const gameEntries = game.data?.entradas ?? [];
  const visibleGlobal = globalEntries.filter((entry) => entry.nome.toLowerCase().includes(term));
  const visibleGame = gameEntries.filter((entry) => entry.nome.toLowerCase().includes(term));
  const selectedGlobal = isGlobal ? globalEntries.find((entry) => entry.jogador_id === selectedPlayer) : undefined;
  const selectedGame = !isGlobal ? gameEntries.find((entry) => entry.jogador_id === selectedPlayer) : undefined;
  const current = isGlobal ? global : game;
  const total = isGlobal ? globalEntries.length : gameEntries.length;
  const visible = isGlobal ? visibleGlobal.length : visibleGame.length;

  return (
    <div className="client-page">
      <PageHeading title="Ranking" detail="Compare os resultados por jogo e consulte os detalhes de cada jogador." />
      <div className="page-filters">
        <label>{t("Jogo")}
          <select value={scope} onChange={(event) => { setScope(event.target.value); setSelectedPlayer(null); }}>
            <option value={GLOBAL}>{t("Ranking Global")}</option>
            {rooms.map((item) => <option key={item.id} value={item.id}>{item.title}</option>)}
          </select>
        </label>
        <label>{t("Jogador")}<input type="search" placeholder={t("Buscar jogador…")} value={query} onChange={(event) => setQuery(event.target.value)} /></label>
        <button type="button" className="secondary-action" onClick={current.reload} disabled={current.status === "loading"}>{t("Atualizar")}</button>
      </div>
      <div className="master-detail">
        <div className="table-scroll">
          <table className="ranking-table">
            <caption>{isGlobal ? t("Ranking Global · soma das notas dos 5 jogos mais jogados") : `${room?.title ?? ""} ${t(" · melhor placar de cada jogador")}`}</caption>
            <thead>
              {isGlobal
                ? <tr><th scope="col">{t("Posição")}</th><th scope="col">{t("Jogador")}</th><th scope="col">{t("Jogos")}</th><th scope="col">{t("Pontos")}</th></tr>
                : <tr><th scope="col">{t("Posição")}</th><th scope="col">{t("Jogador")}</th><th scope="col">{t("Placar")}</th><th scope="col">{t("Nota")}</th></tr>}
            </thead>
            <tbody>
              {isGlobal && visibleGlobal.map((entry) => (
                <tr key={entry.jogador_id} className={selectedPlayer === entry.jogador_id ? "selected-row" : ""}>
                  <td>{String(entry.posicao).padStart(2, "0")}</td>
                  <td><button className="text-action" onClick={() => setSelectedPlayer(entry.jogador_id)}>{entry.nome}</button></td>
                  <td>{entry.jogos_contados}</td>
                  <td className="score-value">{entry.pontos.toFixed(2)}</td>
                </tr>
              ))}
              {!isGlobal && visibleGame.map((entry) => (
                <tr key={entry.jogador_id} className={selectedPlayer === entry.jogador_id ? "selected-row" : ""}>
                  <td>{String(entry.posicao).padStart(2, "0")}</td>
                  <td><button className="text-action" onClick={() => setSelectedPlayer(entry.jogador_id)}>{entry.nome}</button>{!entry.verificada && <span className="muted" title={t("Resultado em revisão")}> *</span>}</td>
                  <td>{entry.score.toLocaleString(locale)}</td>
                  <td className="score-value">{entry.nota.toFixed(2)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <StateMessage status={current.status} empty={total === 0} onRetry={current.reload} />
          {current.status === "ready" && total > 0 && visible === 0 && <p className="empty-state">{t("Nenhum jogador encontrado.")}</p>}
          {!isGlobal && gameEntries.some((entry) => !entry.verificada) && <p className="muted">{t("* Resultado em revisão: conta no ranking até a revisão terminar.")}</p>}
        </div>
        <article className="dashboard-panel">
          <header><h2>{selectedGlobal?.nome ?? selectedGame?.nome ?? t("Detalhes do resultado")}</h2></header>
          {selectedGlobal && (
            <dl className="detail-list">
              <dt>{t("Posição")}</dt><dd>{selectedGlobal.posicao}</dd>
              <dt>{t("Pontos")}</dt><dd className="score-value">{selectedGlobal.pontos.toFixed(2)}</dd>
              <dt>{t("Runs")}</dt><dd>{selectedGlobal.runs}</dd>
              {selectedGlobal.jogos.map((item) => <span key={item.jogo_id} style={{ display: "contents" }}><dt>{item.titulo}</dt><dd className="score-value">{item.nota.toFixed(2)}</dd></span>)}
            </dl>
          )}
          {selectedGame && (
            <dl className="detail-list">
              <dt>{t("Jogo")}</dt><dd>{room?.title}</dd>
              <dt>{t("Placar bruto")}</dt><dd>{selectedGame.score.toLocaleString(locale)}</dd>
              <dt>{t("Vidas perdidas")}</dt><dd>{selectedGame.vidas_perdidas ?? "—"}</dd>
              <dt>{t("Continues")}</dt><dd>{selectedGame.continues === 0 ? t("1CC · sem continues") : selectedGame.continues}</dd>
              <dt>{t("Nota normalizada")}</dt><dd className="score-value">{selectedGame.nota.toFixed(2)}</dd>
              <dt>{t("Status")}</dt><dd>{selectedGame.verificada ? t("Verificado") : t("Em revisão")}</dd>
            </dl>
          )}
          {!selectedGlobal && !selectedGame && <p className="muted">{t("Selecione um jogador para consultar o resultado.")}</p>}
        </article>
      </div>
    </div>
  );
}
