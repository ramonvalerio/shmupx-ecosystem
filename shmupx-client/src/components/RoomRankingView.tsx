import { useI18n } from "../i18n";
import { useMemo, useState } from "react";
import { gerarRankingSalaMock } from "../data/mockRoomRanking";
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
  onBack: () => void;
}

/**
 * Ranking do jogo, aberto na aba da sala. Local: runs deste jogo observadas nesta sessão (o
 * placar ainda não é exposto a esta tela). Online: dados de demonstração até o backend publicar
 * o ranking.
 */
export default function RoomRankingView({ room, runs, onBack }: RoomRankingViewProps) {
  const { t, locale } = useI18n();
  const [scope, setScope] = useState<Scope>("local");
  const [mode, setMode] = useState<Mode>("highscore");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);

  const localRuns = useMemo(
    () => runs.filter((run) => run.gameId === room.id).sort((a, b) => b.startedAt - a.startedAt),
    [runs, room.id],
  );
  const online = useMemo(() => {
    const entries = gerarRankingSalaMock(room.id);
    if (mode === "caravan") return [];
    const pool = mode === "1cc" ? entries.filter((entry) => entry.continuesUsados === 0) : entries;
    return [...pool].sort((a, b) => b.maiorPlacarBruto - a.maiorPlacarBruto);
  }, [room.id, mode]);
  const term = query.trim().toLowerCase();
  const visibleOnline = online
    .map((entry, index) => ({ entry, position: index + 1 }))
    .filter(({ entry }) => entry.jogadorNome.toLowerCase().includes(term));

  const selectedRun = scope === "local" ? localRuns.find((run) => run.id === selected) : undefined;
  const selectedEntry = scope === "online" ? online.find((entry) => entry.jogadorNome === selected) : undefined;

  function changeScope(next: Scope) {
    setScope(next);
    setSelected(null);
  }

  return (
    <section className="room-ranking" aria-label={t("Ranking de {game}", { game: room.title })}>
      <header className="room-ranking-header">
        <div>
          <h1>{room.title}</h1>
          <p>{t("RANKING DA SALA")}</p>
        </div>
        <button type="button" className="room-action" onClick={onBack}>{t("← Voltar para a sala")}</button>
      </header>

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
        {scope === "online" && <span className="sample-chip">{t("DADOS DE DEMONSTRAÇÃO")}</span>}
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
                <span className="rank-score">—</span>
                <span className="rank-duration">{run.endedAt ? formatDuration(run.endedAt - run.startedAt) : t("Em andamento")}</span>
              </button>
            ))}
            <p className="muted room-ranking-note">{t("O placar de cada run aparece aqui quando o backend publicar o resultado aprovado.")}</p>
          </>
        ) : (
          <>
            <div className="room-ranking-row is-head" aria-hidden="true"><span>{t("Posição")}</span><span>{t("Jogador")}</span><span>{t("Placar")}</span><span>{t("Continues")}</span></div>
            {mode === "caravan" && <p className="empty-state">{t("Ainda não há ranking Caravan para este jogo.")}</p>}
            {mode !== "caravan" && visibleOnline.length === 0 && <p className="empty-state">{t("Nenhum jogador encontrado.")}</p>}
            {visibleOnline.map(({ entry, position }) => (
              <button key={entry.jogadorNome} type="button" className="room-ranking-row" aria-pressed={selected === entry.jogadorNome} onClick={() => setSelected(entry.jogadorNome)}>
                <span className="rank-position">{String(position).padStart(2, "0")}</span>
                <span>{entry.jogadorNome}</span>
                <span className="rank-score">{entry.maiorPlacarBruto.toLocaleString(locale)}</span>
                <span className="rank-duration">{entry.continuesUsados === 0 ? t("1CC") : entry.continuesUsados}</span>
              </button>
            ))}
          </>
        )}
      </div>

      <aside className="room-ranking-details" aria-label={t("Detalhes do registro")}>
        <h2>{t("DETALHES DO REGISTRO")}</h2>
        {selectedRun && (
          <>
            <div className="big-score" aria-label={t("Placar")}>—</div>
            <dl className="detail-list">
              <dt>{t("Início")}</dt><dd>{runDate(selectedRun.startedAt, locale)}</dd>
              <dt>{t("Duração")}</dt><dd>{selectedRun.endedAt ? formatDuration(selectedRun.endedAt - selectedRun.startedAt) : t("Em andamento")}</dd>
              <dt>{t("Status")}</dt><dd>{t(RUN_LABELS[selectedRun.status])}</dd>
            </dl>
            {selectedRun.message && <p className="muted" role="status">{selectedRun.message}</p>}
          </>
        )}
        {selectedEntry && (
          <>
            <p className="room-ranking-player">{selectedEntry.jogadorNome}</p>
            <div className="big-score">{selectedEntry.maiorPlacarBruto.toLocaleString(locale)}</div>
            <dl className="detail-list">
              <dt>{t("Modo de jogo")}</dt><dd>{t(MODES.find((item) => item.id === mode)?.label ?? "")}</dd>
              <dt>{t("Vidas perdidas")}</dt><dd>{selectedEntry.vidasPerdidas}</dd>
              <dt>{t("Continues")}</dt><dd>{selectedEntry.continuesUsados === 0 ? t("1CC · sem continues") : selectedEntry.continuesUsados}</dd>
              <dt>{t("Nota normalizada")}</dt><dd className="score-value">{selectedEntry.notaNormalizada.toFixed(2)}</dd>
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
