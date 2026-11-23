import { useI18n } from "../i18n";
import { useMemo, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { useGameRanking } from "../data/useRanking";
import type { GameRoom } from "../types/room";
import type { SessionRun } from "../types/submission";
import { RUN_LABELS, runDate } from "./AccountPages";

type Scope = "local" | "online";
type Mode = "highscore" | "1cc" | "caravan";

const MODES: { id: Mode; label: string }[] = [
  { id: "highscore", label: "Hi-score" },
  { id: "1cc", label: "1CC" },
  { id: "caravan", label: "Caravan" },
];

/** 236_000 -> "3m 56s". */
function formatDuration(ms: number): string {
  const seconds = Math.max(0, Math.round(ms / 1000));
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return h > 0 ? `${h}h ${m}m` : `${m}m ${String(s).padStart(2, "0")}s`;
}

interface RoomRankingViewProps {
  room: GameRoom;
  runs: SessionRun[];
}

/**
 * Ranking do jogo, aberto na aba da sala. Local: runs deste jogo observadas nesta sessão (o
 * placar ainda não é exposto a esta tela). Online: `GET /api/ranking/jogos/{id}` do backend
 * (melhor placar de cada jogador no modo escolhido). Sem painel de título: o caminho no topo
 * (Minhas coleções › jogo › Ranking) mostra onde se está e volta para a sala.
 */
export default function RoomRankingView({ room, runs }: RoomRankingViewProps) {
  const { t, locale } = useI18n();
  const [scope, setScope] = useState<Scope>("local");
  const [mode, setMode] = useState<Mode>("highscore");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [retryError, setRetryError] = useState<string | null>(null);

  const localRuns = useMemo(
    () => runs.filter((run) => run.gameId === room.id).sort((a, b) => b.startedAt - a.startedAt),
    [runs, room.id],
  );
  const ranking = useGameRanking(scope === "online" ? room.id : "", mode);
  const online = useMemo(() => ranking.data?.entradas ?? [], [ranking.data]);
  const term = query.trim().toLowerCase();
  const visibleOnline = online
    .map((entry) => ({ entry, position: entry.posicao }))
    .filter(({ entry }) => entry.nome.toLowerCase().includes(term));

  const selectedRun = scope === "local" ? localRuns.find((run) => run.id === selected) : undefined;
  const selectedEntry = scope === "online" ? online.find((entry) => entry.jogador_id === selected) : undefined;

  /** Run "Não enviada": analisa o vídeo de novo e envia (ou tira da lista). */
  async function retryRun(runId: string, discard = false) {
    setRetryError(null);
    if (!isTauri()) return;
    try {
      await invoke(discard ? "descartar_run_pendente" : "reanalisar_run", { runId });
      if (discard) setSelected(null);
    } catch (error) {
      setRetryError(typeof error === "string" ? error : String(error));
    }
  }

  function changeScope(next: Scope) {
    setScope(next);
    setSelected(null);
  }

  return (
    <section className="room-ranking" aria-label={t("Ranking de {game}", { game: room.title })}>

      <div className="room-ranking-filters">
        <div className="filter-group">
          <span>{t("ESCOPO")}</span>
          <div className="segmented" role="group" aria-label={t("Escopo do ranking")}>
            <button type="button" aria-pressed={scope === "local"} onClick={() => changeScope("local")}>{t("Local")}</button>
            <button type="button" aria-pressed={scope === "online"} onClick={() => changeScope("online")}>{t("Online")}</button>
          </div>
        </div>
        <div className="filter-group">
          <span>{t("MODO DE JOGO")}</span>
          <div className="segmented" role="group" aria-label={t("Modo de jogo")}>
            {MODES.map((item) => (
              <button key={item.id} type="button" aria-pressed={mode === item.id} disabled={scope === "local"} onClick={() => { setMode(item.id); setSelected(null); }}>
                {t(item.label)}
              </button>
            ))}
          </div>
        </div>
        {scope === "online" && (
          <label className="room-ranking-search">
            <span className="sr-only">{t("Buscar jogador")}</span>
            <input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("Buscar jogador…")} />
          </label>
        )}
        {scope === "online" && <button type="button" className="room-action" onClick={ranking.reload} disabled={ranking.status === "loading"}>{t("Atualizar")}</button>}
      </div>

      <div className="room-ranking-list">
        {scope === "local" ? (
          <>
            <div className="room-ranking-row is-head" aria-hidden="true"><span>{t("Início")}</span><span>{t("Status")}</span><span>{t("Placar")}</span><span>{t("Duração")}</span></div>
            {localRuns.length === 0 && <p className="empty-state">{t("Nenhuma run deste jogo nesta sessão. Inicie o jogo pela sala para registrar uma.")}</p>}
            {localRuns.map((run) => (
              <button key={run.id} type="button" className="room-ranking-row" aria-pressed={selected === run.id} onClick={() => setSelected(run.id)}>
                <span>{runDate(run.startedAt, locale)}</span>
                <span className={`run-badge run-${run.status}`}>{t(RUN_LABELS[run.status])}</span>
                <span className="rank-score">{run.score !== undefined ? run.score.toLocaleString(locale) : "—"}</span>
                <span className="rank-duration">{run.endedAt ? formatDuration(run.endedAt - run.startedAt) : t("Em andamento")}</span>
              </button>
            ))}
            <p className="muted room-ranking-note">{t("O placar de cada run aparece aqui quando o backend publicar o resultado aprovado.")}</p>
          </>
        ) : (
          <>
            <div className="room-ranking-row is-head" aria-hidden="true"><span>{t("Posição")}</span><span>{t("Jogador")}</span><span>{t("Placar")}</span><span>{t("Continues")}</span></div>
            {ranking.status === "loading" && online.length === 0 && <p className="empty-state" role="status">{t("Carregando o ranking…")}</p>}
            {ranking.status === "error" && <p className="empty-state" role="alert">{t("Não foi possível carregar o ranking agora.")} <button type="button" className="text-action" onClick={ranking.reload}>{t("Tentar de novo")}</button></p>}
            {ranking.status === "ready" && online.length === 0 && <p className="empty-state">{t("Nenhum resultado ainda")}</p>}
            {ranking.status === "ready" && online.length > 0 && visibleOnline.length === 0 && <p className="empty-state">{t("Nenhum jogador encontrado.")}</p>}
            {visibleOnline.map(({ entry, position }) => (
              <button key={entry.jogador_id} type="button" className="room-ranking-row" aria-pressed={selected === entry.jogador_id} onClick={() => setSelected(entry.jogador_id)}>
                <span className="rank-position">{String(position).padStart(2, "0")}</span>
                <span>{entry.nome}{!entry.verificada && <span className="muted" title={t("Resultado em revisão")}> *</span>}</span>
                <span className="rank-score">{entry.score.toLocaleString(locale)}</span>
                <span className="rank-duration">{entry.continues === 0 ? t("1CC") : entry.continues}</span>
              </button>
            ))}
          </>
        )}
      </div>

      <aside className="room-ranking-details" aria-label={t("Detalhes do registro")}>
        <h2>{t("DETALHES DO REGISTRO")}</h2>
        {selectedRun && (
          <>
            <div className="big-score" aria-label={t("Placar")}>{selectedRun.score !== undefined ? selectedRun.score.toLocaleString(locale) : "—"}</div>
            <dl className="detail-list">
              <dt>{t("Início")}</dt><dd>{runDate(selectedRun.startedAt, locale)}</dd>
              <dt>{t("Duração")}</dt><dd>{selectedRun.endedAt ? formatDuration(selectedRun.endedAt - selectedRun.startedAt) : t("Em andamento")}</dd>
              <dt>{t("Status")}</dt><dd>{t(RUN_LABELS[selectedRun.status])}</dd>
            </dl>
            {selectedRun.message && <p className="muted" role="status">{selectedRun.message}</p>}
            {selectedRun.status === "failed" && selectedRun.runId && (
              <div className="run-retry">
                <p className="muted">{t("O vídeo desta run está guardado: dá para analisar de novo e enviar o placar.")}</p>
                <button type="button" className="room-action" onClick={() => void retryRun(selectedRun.runId!)}>{t("Analisar de novo")}</button>
                <button type="button" className="text-action" onClick={() => void retryRun(selectedRun.runId!, true)}>{t("Descartar")}</button>
                {retryError && <p className="form-error" role="alert">{retryError}</p>}
              </div>
            )}
          </>
        )}
        {selectedEntry && (
          <>
            <p className="room-ranking-player">{selectedEntry.nome}</p>
            <div className="big-score">{selectedEntry.score.toLocaleString(locale)}</div>
            <dl className="detail-list">
              <dt>{t("Posição")}</dt><dd>{selectedEntry.posicao}</dd>
              <dt>{t("Modo de jogo")}</dt><dd>{t(MODES.find((item) => item.id === mode)?.label ?? "")}</dd>
              <dt>{t("Vidas perdidas")}</dt><dd>{selectedEntry.vidas_perdidas ?? "—"}</dd>
              <dt>{t("Continues")}</dt><dd>{selectedEntry.continues === 0 ? t("1CC · sem continues") : selectedEntry.continues}</dd>
              <dt>{t("Nota normalizada")}</dt><dd className="score-value">{selectedEntry.nota.toFixed(2)}</dd>
              <dt>{t("Runs")}</dt><dd>{selectedEntry.runs}</dd>
              <dt>{t("Status")}</dt><dd>{selectedEntry.verificada ? t("Verificado") : t("Em revisão")}</dd>
              {selectedEntry.conquistado_em && <><dt>{t("Data")}</dt><dd>{new Date(selectedEntry.conquistado_em).toLocaleString(locale, { dateStyle: "short", timeStyle: "short" })}</dd></>}
            </dl>
          </>
        )}
        {!selectedRun && !selectedEntry && <p className="muted">{t("Selecione um registro para ver os detalhes.")}</p>}
        <div className="gameplay-placeholder">
          <span aria-hidden="true">▶</span>
          <p>{t("Gameplay indisponível neste dispositivo.")}</p>
        </div>
      </aside>
    </section>
  );
}
