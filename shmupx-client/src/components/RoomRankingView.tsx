import { useI18n } from "../i18n";
import { useEffect, useMemo, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { useGameRanking } from "../data/useRanking";
import { avatarOf, usePublicProfiles } from "../data/publicProfile";
import { useRoomPresence, videoIdFromUrl } from "../data/useRoomPresence";
import { useLive } from "../data/useLive";
import type { GameRankingEntry } from "../types/ranking";
import type { GameMode, GameRoom, PlayOn } from "../types/room";
import type { SessionRun } from "../types/submission";
import { RUN_LABELS } from "./AccountPages";

type Scope = "local" | "online";

const MODES: { id: GameMode; label: string }[] = [
  { id: "highscore", label: "Hi-Score" },
  { id: "1cc", label: "1CC" },
  { id: "caravan", label: "Caravan" },
];
const MODE_LABEL: Record<GameMode, string> = { highscore: "Hi-Score", "1cc": "1CC", caravan: "Caravan" };
const PLATFORM_LABEL: Record<PlayOn, string> = { steam: "Steam", emulator: "Emulador", mister: "MiSTer" };
/** Dígitos do placar no estilo arcade: zeros à esquerda apagados até 8 casas. */
const SCORE_DIGITS = 8;

/** 236_000 -> "3m 56s". */
function formatDuration(ms: number): string {
  const seconds = Math.max(0, Math.round(ms / 1000));
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return h > 0 ? `${h}h ${m}m` : `${m}m ${String(s).padStart(2, "0")}s`;
}

/** "18/11/2026 · 21:14" (no idioma do app). */
function formatWhen(timestamp: number, locale: string): string {
  const date = new Date(timestamp);
  const day = date.toLocaleDateString(locale, { day: "2-digit", month: "2-digit", year: "numeric" });
  const time = date.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit", hour12: false });
  return `${day} · ${time}`;
}

/** "highscore" | "1cc" | "caravan-classic"… do backend para o modo da tela. */
function modeOf(value: string | null | undefined): GameMode | undefined {
  if (!value) return undefined;
  if (value === "highscore" || value === "1cc") return value;
  return value.startsWith("caravan") ? "caravan" : undefined;
}

function platformOf(value: string | null | undefined): PlayOn | undefined {
  return value === "steam" || value === "emulator" || value === "mister" ? value : undefined;
}

function timeOf(value: string | null | undefined): number | undefined {
  if (!value) return undefined;
  const time = Date.parse(value);
  return Number.isFinite(time) ? time : undefined;
}

/** O Rust (`live_abrir`) só abre links `watch?v=` do YouTube no navegador. */
function openYouTube(videoId: string) {
  const url = `https://www.youtube.com/watch?v=${videoId}`;
  if (isTauri()) void invoke("live_abrir", { url }).catch((error) => console.warn("falha ao abrir o YouTube:", error));
  else window.open(url, "_blank", "noopener");
}

/** Linha do ranking já normalizada (online: `GET /api/ranking/jogos/{id}`; local: runs desta sessão). */
interface RankRecord {
  key: string;
  position: number;
  uid?: string;
  you: boolean;
  name: string;
  score?: number;
  /** Quando a gameplay começou (só as runs locais têm; online, ver `submittedAt`). */
  startedAt?: number;
  /** Quando o resultado foi enviado (`conquistado_em`). */
  submittedAt?: number;
  durationMs?: number;
  inProgress?: boolean;
  mode?: GameMode;
  platform?: PlayOn;
  continues?: number;
  livesLost?: number | null;
  runs?: number;
  rating?: number;
  verified?: boolean;
  runStatus?: SessionRun["status"];
  /** Vídeo do YouTube: live no ar agora (presença na sala) ou o vídeo da run (quando o backend expuser). */
  video?: { id: string; live: boolean };
  run?: SessionRun;
}

function ArcadeScore({ value, locale }: { value?: number; locale: string }) {
  if (value === undefined) return <span className="rk-digits is-empty">—</span>;
  const digits = String(Math.max(0, Math.trunc(value)));
  const pad = "0".repeat(Math.max(0, SCORE_DIGITS - digits.length));
  return <span className="rk-digits">{pad && <span className="rk-pad" aria-hidden="true">{pad}</span>}{value.toLocaleString(locale)}</span>;
}

interface RoomRankingViewProps {
  room: GameRoom;
  runs: SessionRun[];
  /** uid e nome do jogador conectado: destacam a linha dele (YOU). */
  uid?: string;
  playerName?: string;
}

/**
 * Ranking do jogo, aberto na aba da sala. Online: `GET /api/ranking/jogos/{id}` do backend (melhor
 * placar de cada jogador no modo escolhido), com pódio do Top 3 e a classificação completa. Local:
 * runs deste jogo observadas nesta sessão. Avatar e nick vêm de `publicProfiles/{uid}`; o LIVE, da
 * presença na sala (quem está transmitindo agora). Campos que o backend ainda não envia (plataforma,
 * início da gameplay, duração e vídeo da run) ficam de fora em vez de inventados.
 */
export default function RoomRankingView({ room, runs, uid, playerName }: RoomRankingViewProps) {
  const { t, locale } = useI18n();
  const [scope, setScope] = useState<Scope>("online");
  const [mode, setMode] = useState<GameMode>("highscore");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [retryError, setRetryError] = useState<string | null>(null);
  const [thumbFailed, setThumbFailed] = useState<string | null>(null);

  const ranking = useGameRanking(scope === "online" ? room.id : "", mode);
  const online = useMemo(() => ranking.data?.entradas ?? [], [ranking.data]);
  const localRuns = useMemo(
    () => runs.filter((run) => run.gameId === room.id).sort((a, b) => b.startedAt - a.startedAt),
    [runs, room.id],
  );

  // Quem está ao vivo nesta sala agora (o próprio jogador vem do estado local da live).
  const presence = useRoomPresence(room.id, uid ?? "");
  const live = useLive();
  const myLiveId = live.state.status === "live" && live.state.game_id === room.id ? videoIdFromUrl(live.state.watch_url) : null;
  const liveByUid = useMemo(() => {
    const map = new Map<string, string>();
    for (const player of presence) if (player.live) map.set(player.uid, player.live.videoId);
    if (uid && myLiveId) map.set(uid, myLiveId);
    return map;
  }, [presence, uid, myLiveId]);

  const profiles = usePublicProfiles([uid ?? "", ...online.map((entry) => entry.jogador_id)]);
  const handleOf = (id?: string) => (id ? profiles.get(id)?.handle : undefined);

  const onlineRecords = useMemo<RankRecord[]>(() => online.map((entry: GameRankingEntry) => {
    const liveId = liveByUid.get(entry.jogador_id);
    const vodId = videoIdFromUrl(entry.youtube_url);
    return {
      key: entry.jogador_id,
      position: entry.posicao,
      uid: entry.jogador_id,
      you: entry.jogador_id === uid,
      name: entry.nome,
      score: entry.score,
      startedAt: timeOf(entry.iniciada_em),
      submittedAt: timeOf(entry.conquistado_em),
      durationMs: typeof entry.duracao_segundos === "number" ? entry.duracao_segundos * 1000 : undefined,
      mode: modeOf(entry.modo) ?? mode,
      platform: platformOf(entry.plataforma),
      continues: entry.continues,
      livesLost: entry.vidas_perdidas,
      runs: entry.runs,
      rating: entry.nota,
      verified: entry.verificada,
      video: liveId ? { id: liveId, live: true } : vodId ? { id: vodId, live: false } : undefined,
    };
  }), [online, liveByUid, uid, mode]);

  const localRecords = useMemo<RankRecord[]>(() => localRuns.map((run, index) => ({
    key: run.id,
    position: index + 1,
    uid,
    you: true,
    name: playerName || t("Você"),
    score: run.score,
    startedAt: run.startedAt,
    durationMs: run.endedAt ? run.endedAt - run.startedAt : undefined,
    inProgress: !run.endedAt,
    runStatus: run.status,
    video: run.status === "recording" && myLiveId ? { id: myLiveId, live: true } : undefined,
    run,
  })), [localRuns, uid, playerName, myLiveId, t]);

  const records = scope === "online" ? onlineRecords : localRecords;
  const term = query.trim().toLowerCase();
  const visible = scope === "online" && term
    ? records.filter((record) => record.name.toLowerCase().includes(term) || (handleOf(record.uid) ?? "").toLowerCase().includes(term))
    : records;
  const podium = scope === "online" ? onlineRecords.filter((record) => record.position <= 3) : [];
  const total = scope === "online" ? ranking.data?.total ?? online.length : localRecords.length;

  // Sem escolha: o 1º lugar (ou a run mais recente) já aparece nos detalhes.
  const current = records.find((record) => record.key === selected) ?? (selected === null ? visible[0] : undefined);
  useEffect(() => { setThumbFailed(null); }, [current?.key, current?.video?.id]);

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
    setQuery("");
  }

  const updatedAt = timeOf(ranking.data?.atualizado_em);
  const modeLabel = t(MODE_LABEL[mode]);

  /** Nick (handle do perfil público) em destaque; o nome de exibição embaixo. */
  function identity(record: RankRecord) {
    const handle = handleOf(record.uid);
    return { nick: handle || record.name, real: handle && handle !== record.name ? record.name : undefined };
  }

  function whenOf(record: RankRecord) {
    const time = record.startedAt ?? record.submittedAt;
    return time !== undefined ? formatWhen(time, locale) : undefined;
  }

  function statusOf(record: RankRecord): { label: string; tone: string } {
    if (record.runStatus) return { label: t(RUN_LABELS[record.runStatus]), tone: record.runStatus };
    return record.verified ? { label: t("Verificado"), tone: "verified" } : { label: t("Em revisão"), tone: "review" };
  }

  function videoButton(record: RankRecord, compact: boolean) {
    const video = record.video;
    if (!video) {
      return compact
        ? <span className="rk-yt is-off" title={t("Sem vídeo no YouTube")} aria-hidden="true"><PlayIcon /></span>
        : null;
    }
    const label = video.live ? t("Assistir à live de {player} no YouTube", { player: identity(record).nick }) : t("Assistir ao vídeo de {player} no YouTube", { player: identity(record).nick });
    return (
      <button type="button" className={`rk-yt ${video.live ? "is-live" : "is-vod"}${compact ? "" : " is-wide"}`} title={label} aria-label={label}
        onClick={(event) => { event.stopPropagation(); openYouTube(video.id); }}>
        {video.live ? <><span className="rk-live-dot" aria-hidden="true" />LIVE</> : <><PlayIcon />{!compact && "VOD"}</>}
      </button>
    );
  }

  const PLACES = [1, 2, 3] as const;
  const PLACE_CLASS: Record<number, string> = { 1: "is-first", 2: "is-second", 3: "is-third" };

  return (
    <section className="room-ranking rk" aria-label={t("Ranking de {game}", { game: room.title })}>

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
        <div className="rk-meta" aria-live="polite">
          <b>{scope === "online" ? t("{count} jogadores", { count: total }) : t("{count} runs nesta sessão", { count: total })}</b>
          {scope === "online" && updatedAt !== undefined && <span>{t("Atualizado às {time}", { time: new Date(updatedAt).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit", hour12: false }) })}</span>}
        </div>
      </div>

      <div className="rk-board">
        {podium.length > 0 && (
          <div className="rk-podium-wrap">
            <h2 className="rk-title">{t("Top 3 · {mode}", { mode: modeLabel })}</h2>
            <ol className="rk-podium" aria-label={t("Top 3 · {mode}", { mode: modeLabel })}>
              {PLACES.map((place) => {
                const record = podium.find((item) => item.position === place);
                if (!record) {
                  return (
                    <li key={place} className={`rk-card ${PLACE_CLASS[place]} is-vacant`}>
                      <span className="rk-medal" aria-hidden="true">{String(place).padStart(2, "0")}</span>
                      <span className="rk-vacant-avatar" aria-hidden="true">?</span>
                      <p className="rk-vacant-text">{t("Lugar vago")}</p>
                      <small>{t("Jogue Ranked Mode para entrar no pódio.")}</small>
                    </li>
                  );
                }
                const { nick, real } = identity(record);
                const when = whenOf(record);
                return (
                  <li key={record.key} className={`rk-card ${PLACE_CLASS[place]}${record.you ? " is-you" : ""}${current?.key === record.key ? " is-selected" : ""}`}>
                    <button type="button" className="rk-hit" aria-pressed={current?.key === record.key}
                      aria-label={t("{position}º lugar: {player}, {score} pontos", { position: place, player: nick, score: record.score?.toLocaleString(locale) ?? "—" })}
                      onClick={() => setSelected(record.key)} />
                    <span className="rk-medal" aria-hidden="true">{String(place).padStart(2, "0")}</span>
                    {record.you && <span className="rk-you">{t("VOCÊ")}</span>}
                    <img className="rk-avatar" src={avatarOf(profiles, record.uid)} alt="" />
                    <h3 className="rk-nick">{nick}</h3>
                    {real && <p className="rk-real">{real}</p>}
                    <div className="rk-score"><ArcadeScore value={record.score} locale={locale} /></div>
                    <div className="rk-chips">
                      {record.mode && <span className="rk-chip is-mode">{t(MODE_LABEL[record.mode])}</span>}
                      {record.platform && <span className="rk-chip">{t(PLATFORM_LABEL[record.platform])}</span>}
                      {record.mode !== "1cc" && record.continues === 0 && <span className="rk-chip is-cc">1CC</span>}
                    </div>
                    <div className="rk-card-foot">
                      {when && <span className="rk-when">{when}</span>}
                      {videoButton(record, false)}
                    </div>
                  </li>
                );
              })}
            </ol>
          </div>
        )}

        <div className="room-ranking-list rk-standings">
          <h2 className="rk-title">{scope === "online" ? t("Classificação completa") : t("Runs desta sessão")}</h2>
          <div className="rk-row is-head" aria-hidden="true">
            <span>#</span><span>{t("Jogador")}</span><span className="rk-col-when">{scope === "online" ? t("Data") : t("Início da run")}</span>
            <span className="rk-col-mode">{scope === "online" ? t("Modo") : t("Status")}</span>
            <span className="rk-col-plat">{scope === "online" ? t("Plataforma") : t("Duração")}</span>
            <span className="rk-col-yt">{t("Live")}</span><span className="rk-col-score">{scope === "online" ? modeLabel : t("Placar")}</span>
          </div>
          {scope === "local" && localRecords.length === 0 && <p className="empty-state">{t("Nenhuma run deste jogo nesta sessão. Inicie o jogo pela sala para registrar uma.")}</p>}
          {scope === "online" && ranking.status === "loading" && online.length === 0 && (
            <div className="rk-skeleton" role="status" aria-label={t("Carregando o ranking…")}>
              {[0, 1, 2, 3, 4].map((index) => <span key={index} />)}
            </div>
          )}
          {scope === "online" && ranking.status === "error" && <p className="empty-state" role="alert">{t("Não foi possível carregar o ranking agora.")} <button type="button" className="text-action" onClick={ranking.reload}>{t("Tentar de novo")}</button></p>}
          {scope === "online" && ranking.status === "ready" && online.length === 0 && (
            <div className="rk-empty">
              <strong>{t("Nenhum resultado ainda")}</strong>
              <p>{t("Seja o primeiro: jogue {game} em Ranked Mode no modo {mode}.", { game: room.title, mode: modeLabel })}</p>
            </div>
          )}
          {scope === "online" && online.length > 0 && visible.length === 0 && <p className="empty-state">{t("Nenhum jogador encontrado.")}</p>}
          <ol className="rk-list">
            {visible.map((record) => {
              const { nick, real } = identity(record);
              const when = whenOf(record);
              const status = statusOf(record);
              const isSelected = current?.key === record.key;
              const rankClass = scope === "online" && record.position <= 3 ? ` rank-${record.position}` : "";
              return (
                <li key={record.key} className={`rk-row${rankClass}${record.you && scope === "online" ? " is-you" : ""}${isSelected ? " is-selected" : ""}`}>
                  <button type="button" className="rk-hit" aria-pressed={isSelected}
                    aria-label={scope === "online"
                      ? t("{position}º lugar: {player}, {score} pontos", { position: record.position, player: nick, score: record.score?.toLocaleString(locale) ?? "—" })
                      : t("Run de {when}: {status}", { when: when ?? "—", status: status.label })}
                    onClick={() => setSelected(record.key)} />
                  <span className="rk-pos">{String(record.position).padStart(2, "0")}</span>
                  <span className="rk-player">
                    <img src={avatarOf(profiles, record.uid)} alt="" />
                    <span className="rk-who">
                      <strong>{nick}{scope === "online" && record.verified === false && <span className="rk-review" title={t("Resultado em revisão")}> *</span>}</strong>
                      {(real || record.you) && <small>{[real, record.you && scope === "online" ? t("Você") : undefined].filter(Boolean).join(" · ")}</small>}
                    </span>
                  </span>
                  <span className="rk-col-when rk-when">{when ?? "—"}</span>
                  <span className="rk-col-mode">
                    {scope === "online"
                      ? record.mode && <span className="rk-chip is-mode">{t(MODE_LABEL[record.mode])}</span>
                      : <span className={`run-badge run-${status.tone}`}>{status.label}</span>}
                  </span>
                  <span className="rk-col-plat">
                    {scope === "online"
                      ? record.platform ? <span className="rk-chip">{t(PLATFORM_LABEL[record.platform])}</span> : <span className="rk-dash" title={t("Plataforma ainda não informada pelo servidor")}>—</span>
                      : <span className="rk-when">{record.inProgress ? t("Em andamento") : record.durationMs !== undefined ? formatDuration(record.durationMs) : "—"}</span>}
                  </span>
                  <span className="rk-col-yt">{videoButton(record, true)}</span>
                  <span className="rk-col-score rk-row-score"><ArcadeScore value={record.score} locale={locale} /></span>
                </li>
              );
            })}
          </ol>
          {scope === "local" && localRecords.length > 0 && <p className="muted room-ranking-note">{t("O placar de cada run aparece aqui quando o backend publicar o resultado aprovado.")}</p>}
        </div>
      </div>

      <aside className="room-ranking-details rk-details" aria-label={t("Detalhes do registro")}>
        <h2>{t("DETALHES DO REGISTRO")}</h2>
        {current ? (() => {
          const { nick, real } = identity(current);
          const status = statusOf(current);
          const thumbOk = current.video && thumbFailed !== current.video.id;
          return (
            <>
              <div className={`rk-thumb${current.video ? "" : " is-empty"}`}>
                {current.video ? (
                  <button type="button" className="rk-thumb-play" onClick={() => openYouTube(current.video!.id)}
                    aria-label={current.video.live ? t("Assistir à live no YouTube") : t("Assistir no YouTube")}>
                    {thumbOk && <img src={`https://i.ytimg.com/vi/${current.video.id}/hqdefault.jpg`} alt="" onError={() => setThumbFailed(current.video!.id)} />}
                    <span className="rk-play" aria-hidden="true"><PlayIcon /></span>
                    {current.video.live && <span className="rk-thumb-live"><span className="rk-live-dot" aria-hidden="true" />LIVE</span>}
                    <span className="rk-thumb-cap">{room.title}{current.mode ? ` · ${t(MODE_LABEL[current.mode])}` : ""}</span>
                  </button>
                ) : (
                  <div className="rk-thumb-placeholder">
                    <span aria-hidden="true"><PlayIcon /></span>
                    <p>{t("Sem vídeo do YouTube para este registro.")}</p>
                  </div>
                )}
              </div>

              <div className="rk-detail-player">
                <img src={avatarOf(profiles, current.uid)} alt="" />
                <div>
                  <strong>{nick}</strong>
                  <small>{[real, current.you ? t("Você") : undefined].filter(Boolean).join(" · ") || "\u00a0"}</small>
                </div>
              </div>

              <div className="rk-big-score" aria-label={t("Placar")}>
                <span className="rk-big-label">{current.mode ? t(MODE_LABEL[current.mode]) : t("Placar")}</span>
                <ArcadeScore value={current.score} locale={locale} />
              </div>

              {(current.mode || current.platform || current.continues === 0) && (
                <div className="rk-chips is-left">
                  {current.mode && <span className="rk-chip is-mode">{t(MODE_LABEL[current.mode])}</span>}
                  {current.platform && <span className="rk-chip">{t(PLATFORM_LABEL[current.platform])}</span>}
                  {current.continues === 0 && <span className="rk-chip is-cc">{t("1CC · sem continues")}</span>}
                </div>
              )}

              <dl className="rk-stats">
                {current.startedAt !== undefined && <div><dt>{t("Início da run")}</dt><dd className="is-mono">{formatWhen(current.startedAt, locale)}</dd></div>}
                {current.startedAt === undefined && current.submittedAt !== undefined && <div><dt>{t("Enviado em")}</dt><dd className="is-mono">{formatWhen(current.submittedAt, locale)}</dd></div>}
                {(current.durationMs !== undefined || current.inProgress) && <div><dt>{t("Duração")}</dt><dd>{current.inProgress ? t("Em andamento") : formatDuration(current.durationMs!)}</dd></div>}
                {current.platform && <div><dt>{t("Plataforma")}</dt><dd>{t(PLATFORM_LABEL[current.platform])}</dd></div>}
                {current.continues !== undefined && <div><dt>{t("Continues")}</dt><dd>{current.continues === 0 ? t("0 · 1CC") : current.continues}</dd></div>}
                {scope === "online" && <div><dt>{t("Posição")}</dt><dd className="is-mono">{String(current.position).padStart(2, "0")} / {String(total).padStart(2, "0")}</dd></div>}
                <div><dt>{t("Status")}</dt><dd className={`rk-status is-${status.tone}`}>{status.label}</dd></div>
                {current.livesLost !== undefined && current.livesLost !== null && <div><dt>{t("Vidas perdidas")}</dt><dd>{current.livesLost}</dd></div>}
                {current.runs !== undefined && <div><dt>{t("Runs")}</dt><dd>{current.runs}</dd></div>}
                {current.rating !== undefined && <div><dt>{t("Nota normalizada")}</dt><dd className="is-mono">{current.rating.toFixed(2)}</dd></div>}
              </dl>

              {current.run?.message && <p className="muted" role="status">{current.run.message}</p>}
              {current.run?.status === "failed" && current.run.runId && (
                <div className="run-retry">
                  <p className="muted">{t("O vídeo desta run está guardado: dá para analisar de novo e enviar o placar.")}</p>
                  <button type="button" className="room-action" onClick={() => void retryRun(current.run!.runId!)}>{t("Analisar de novo")}</button>
                  <button type="button" className="text-action" onClick={() => void retryRun(current.run!.runId!, true)}>{t("Descartar")}</button>
                  {retryError && <p className="form-error" role="alert">{retryError}</p>}
                </div>
              )}

              {current.video && (
                <div className="rk-cta">
                  <p>{current.video.live ? t("Ao vivo agora no YouTube") : t("Vídeo da run no YouTube")}</p>
                  <button type="button" className="rk-watch" onClick={() => openYouTube(current.video!.id)}>
                    <PlayIcon />{current.video.live ? t("Assistir à live") : t("Assistir no YouTube")}
                  </button>
                </div>
              )}
            </>
          );
        })() : (
          <p className="muted">{t("Selecione um registro para ver os detalhes.")}</p>
        )}
      </aside>
    </section>
  );
}

function PlayIcon() {
  return <svg className="rk-play-icon" viewBox="0 0 12 12" aria-hidden="true" focusable="false"><path d="M3 1.5v9l8-4.5z" /></svg>;
}
