import { useI18n } from "../i18n";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { useGameRanking } from "../data/useRanking";
import { avatarOf, usePublicProfiles } from "../data/publicProfile";
import { useRoomPresence, videoIdFromUrl } from "../data/useRoomPresence";
import { useLive } from "../data/useLive";
import type { GameRankingEntry } from "../types/ranking";
import type { GameMode, GameRoom, PlayOn } from "../types/room";
import type { SessionRun } from "../types/submission";
import { RUN_LABELS } from "./AccountPages";
import { useLocalRanking } from "../data/useLocalRanking";

type Scope = "local" | "online";

const MODES: { id: GameMode; label: string }[] = [
  { id: "highscore", label: "Hi-Score" },
  { id: "1cc", label: "1CC" },
  { id: "caravan", label: "Caravan" },
];
const MODE_LABEL: Record<GameMode, string> = { highscore: "Hi-Score", "1cc": "1CC", caravan: "Caravan" };
const PLATFORM_LABEL: Record<PlayOn, string> = { steam: "Steam", emulator: "Emulador", mister: "MiSTer" };
/** Placar fixo de fliperama: o Top 10 sempre aparece, com as vagas em "_ _ _". */
const SLOTS = 10;

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
  /** Run do ranking local: ficou também no ranking online (o recorde) ou só aqui. */
  localStatus?: "online" | "local";
  /** Vídeo do YouTube: live no ar agora (presença na sala) ou o vídeo da run (quando o backend expuser). */
  video?: { id: string; live: boolean };
  run?: SessionRun;
}

/** Posição no estilo placar de fliperama: 1ST, 2ND, 3RD, 4TH… */
function Ordinal({ n }: { n: number }) {
  // Run local ainda sem resultado (em andamento, analisando, não enviada): sem posição.
  if (n <= 0) return <>—</>;
  const tens = n % 100;
  const suffix = tens >= 11 && tens <= 13 ? "TH" : ({ 1: "ST", 2: "ND", 3: "RD" } as Record<number, string>)[n % 10] ?? "TH";
  return <>{n}<sup>{suffix}</sup></>;
}

function score(value: number | undefined, locale: string) {
  return value === undefined ? "—" : Math.trunc(value).toLocaleString(locale);
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
 * placar de cada jogador no modo escolhido), com a faixa do recorde e o placar do Top 10. Local:
 * o ranking local do jogador (todas as runs válidas dele neste PC, do maior placar para o menor;
 * `useLocalRanking`), com as runs desta sessão ainda sem resultado no topo. Avatar e nick vêm de `publicProfiles/{uid}`; o LIVE, da
 * presença na sala (quem está transmitindo agora). Campos que o backend ainda não envia (plataforma,
 * início da gameplay, duração e vídeo da run) aparecem como "—" em vez de inventados.
 * Visual (proposta v2 aprovada): faixa do recorde com a placa 1ST, placar de fliperama com o Top 10
 * fixo e painel de detalhes em tabela; tudo chapado, linhas de 1px e sem brilho.
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
  // Runs desta sessão ainda sem resultado aceito (em andamento, analisando, não enviada); as aceitas
  // já estão no ranking local.
  const localRuns = useMemo(
    () => runs.filter((run) => run.gameId === room.id && run.status !== "submitted").sort((a, b) => b.startedAt - a.startedAt),
    [runs, room.id],
  );
  const localRanking = useLocalRanking(uid, room.id, scope === "local");

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

  const localRecords = useMemo<RankRecord[]>(() => [...localRuns.map((run) => ({
    key: run.id,
    position: 0,
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
  })), ...localRanking.map((entry, index) => ({
    key: entry.run_id,
    position: index + 1,
    uid,
    you: true,
    name: playerName || t("Você"),
    score: entry.score,
    submittedAt: entry.quando,
    mode: modeOf(entry.modo) ?? "highscore",
    continues: entry.continues,
    livesLost: entry.vidas_perdidas,
    localStatus: (entry.online ? "online" : "local") as RankRecord["localStatus"],
  }))], [localRuns, localRanking, uid, playerName, myLiveId, t]);

  const records = scope === "online" ? onlineRecords : localRecords;
  const term = query.trim().toLowerCase();
  const visible = scope === "online" && term
    ? records.filter((record) => record.name.toLowerCase().includes(term) || (handleOf(record.uid) ?? "").toLowerCase().includes(term))
    : records;
  const leader = scope === "online" ? onlineRecords.find((record) => record.position === 1) : undefined;
  const runnerUp = scope === "online" ? onlineRecords.find((record) => record.position === 2) : undefined;
  const total = scope === "online" ? ranking.data?.total ?? online.length : localRanking.length;

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

  function statusOf(record: RankRecord): { label: string; tone: string } {
    if (record.runStatus) return { label: t(RUN_LABELS[record.runStatus]), tone: record.runStatus };
    if (record.localStatus) return record.localStatus === "online" ? { label: t("No ranking online"), tone: "verified" } : { label: t("Só no ranking local"), tone: "review" };
    return record.verified ? { label: t("Verificado"), tone: "verified" } : { label: t("Em revisão"), tone: "review" };
  }

  const online_ = scope === "online";
  /** Coluna de data: início da gameplay quando o backend mandar; senão, quando o placar foi enviado. */
  const anyStarted = !online_ || records.some((record) => record.startedAt !== undefined);
  const dateLabel = anyStarted ? t("Início da run") : t("Enviado em");
  function dateOf(record: RankRecord) {
    const time = anyStarted ? record.startedAt : record.submittedAt;
    return time !== undefined ? formatWhen(time, locale) : undefined;
  }
  const showSlots = online_ && !term && ranking.status === "ready" && online.length > 0;
  const emptySlots = showSlots ? Math.max(0, SLOTS - visible.length) : 0;
  const lastPosition = visible.length > 0 ? visible[visible.length - 1].position : 0;
  const dash = <span className="rk-none">—</span>;

  function platformText(record: RankRecord) {
    return record.platform ? t(PLATFORM_LABEL[record.platform]) : undefined;
  }

  function continuesCell(record: RankRecord) {
    if (record.continues === undefined) return dash;
    return record.continues === 0 ? <>0<em>· 1CC</em></> : <>{record.continues}</>;
  }

  /** Ícone do vídeo na tabela: LIVE (vermelho), VOD (botão) ou apagado quando a run não tem vídeo. */
  function videoCell(record: RankRecord) {
    const video = record.video;
    if (!video) return <span className="rk-yt is-off" title={t("Sem vídeo no YouTube")}><YouTubeIcon /></span>;
    const label = video.live ? t("Assistir à live de {player} no YouTube", { player: identity(record).nick }) : t("Assistir ao vídeo de {player} no YouTube", { player: identity(record).nick });
    return (
      <button type="button" className={`rk-yt ${video.live ? "is-live" : "is-vod"}`} title={label} aria-label={label}
        onClick={(event) => { event.stopPropagation(); openYouTube(video.id); }}>
        {video.live ? <><span className="rk-live-dot" aria-hidden="true" />LIVE</> : <><YouTubeIcon />VOD</>}
      </button>
    );
  }

  return (
    <section className="room-ranking rk" aria-label={t("Ranking de {game}", { game: room.title })}>

      <div className="room-ranking-filters" data-tutorial="ranking-filters">
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
        {leader && (() => {
          const { nick, real } = identity(leader);
          const handle = handleOf(leader.uid);
          const when = dateOf(leader);
          const gap = leader.score !== undefined && runnerUp?.score !== undefined ? leader.score - runnerUp.score : undefined;
          const isSelected = current?.key === leader.key;
          return (
            <section className={`rk-leader${isSelected ? " is-selected" : ""}`} aria-label={t("Recorde atual")}>
              <button type="button" className="rk-hit" aria-pressed={isSelected}
                aria-label={t("{position}º lugar: {player}, {score} pontos", { position: 1, player: nick, score: score(leader.score, locale) })}
                onClick={() => setSelected(leader.key)} />
              <div className="rk-plate" aria-hidden="true"><span className="rk-plate-rank"><Ordinal n={1} /></span><small>{t("RECORDE")}</small><img className="rk-plate-medal" src="/img/medals/medal-1.png" alt="" draggable={false} /></div>
              <img className="rk-leader-avatar" src={avatarOf(profiles, leader.uid)} alt="" />
              <div className="rk-leader-who">
                <span className="rk-kicker"><i aria-hidden="true" />{room.title} · {modeLabel} · {t("Online")}</span>
                <h2>{nick}</h2>
                <span className="rk-handle">{[real ?? (handle ? `@${handle}` : undefined), leader.you ? t("Você") : undefined].filter(Boolean).join(" · ") || "\u00a0"}</span>
                <div className="rk-tags">
                  {leader.mode && <span className="rk-tag is-mode">{t(MODE_LABEL[leader.mode])}</span>}
                  {leader.continues !== undefined && <span className="rk-tag">{leader.continues === 0 ? t("0 continues · 1CC") : t("{count} continues", { count: leader.continues })}</span>}
                  {leader.verified === false && <span className="rk-tag is-review">* {t("Em revisão")}</span>}
                </div>
              </div>
              <div className="rk-leader-score">
                <span className="rk-label">{modeLabel}</span>
                <span className="rk-leader-num">{score(leader.score, locale)}</span>
                {gap !== undefined && <span className="rk-gap"><b>+{gap.toLocaleString(locale)}</b> {t("à frente do 2º lugar")}</span>}
              </div>
              <div className="rk-leader-meta">
                <div><span>{dateLabel}</span><span>{when ?? dash}</span></div>
                <div><span>{t("Plataforma")}</span><span>{platformText(leader) ?? dash}</span></div>
                <div><span>{t("Vídeo")}</span><span>{leader.video ? videoCell(leader) : <span className="rk-yt is-off is-text"><YouTubeIcon />{t("Sem vídeo")}</span>}</span></div>
              </div>
            </section>
          );
        })()}

        <div className="room-ranking-list rk-table">
          <header className="rk-table-head">
            <h2>{online_ ? t("Classificação · {mode}", { mode: modeLabel }) : t("Runs desta sessão")}</h2>
            {online_ && online.length > 0 && <span>{total <= SLOTS ? t("Top {slots} · {count} de {slots} posições ocupadas", { slots: SLOTS, count: total }) : t("{count} jogadores", { count: total })}</span>}
          </header>
          <div className="rk-row is-head" aria-hidden="true">
            <span className="rk-pos">{t("Pos.")}</span><span>{t("Jogador")}</span>
            <span className="rk-r">{online_ ? modeLabel : t("Placar")}</span>
            <span className="rk-c">{online_ ? t("Modo") : t("Status")}</span>
            <span className="rk-c">{t("Continues")}</span>
            <span className="rk-c">{online_ ? t("Plataforma") : t("Duração")}</span>
            <span>{dateLabel}</span>
            <span className="rk-c">{t("Vídeo")}</span>
          </div>
          {scope === "local" && localRecords.length === 0 && <p className="empty-state">{t("Nenhuma run deste jogo neste PC ainda. As runs válidas do Ranked ficam guardadas aqui.")}</p>}
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
              const when = dateOf(record);
              const status = statusOf(record);
              const isSelected = current?.key === record.key;
              const place = online_ && record.position <= 3 ? ` is-p${record.position}` : "";
              const sub = real;
              return (
                <li key={record.key} className={`rk-row${place}${isSelected ? " is-selected" : ""}`}>
                  <button type="button" className="rk-hit" aria-pressed={isSelected}
                    aria-label={online_
                      ? t("{position}º lugar: {player}, {score} pontos", { position: record.position, player: nick, score: score(record.score, locale) })
                      : t("Run de {when}: {status}", { when: when ?? "—", status: status.label })}
                    onClick={() => setSelected(record.key)} />
                  <span className="rk-pos">
                    <span><Ordinal n={record.position} /></span>
                    {online_ && record.position <= 3 && <img className="rk-pos-medal" src={`/img/medals/medal-${record.position}.png`} alt="" draggable={false} />}
                  </span>
                  <span className="rk-player">
                    <img src={avatarOf(profiles, record.uid)} alt="" />
                    <span className="rk-who">
                      <strong>{nick}</strong>
                      {online_ && record.verified === false && <span className="rk-review" title={t("Resultado em revisão")}> *</span>}
                      {sub && <small>{sub}</small>}
                    </span>
                    {record.you && online_ && <span className="rk-tag is-you">{t("Você")}</span>}
                  </span>
                  <span className="rk-r rk-score">{score(record.score, locale)}</span>
                  <span className="rk-c">
                    {online_
                      ? record.mode ? <span className="rk-chip">{t(MODE_LABEL[record.mode])}</span> : dash
                      : <span className={`run-badge run-${status.tone}`}>{status.label}</span>}
                  </span>
                  <span className="rk-c rk-cc">{continuesCell(record)}</span>
                  <span className="rk-c">
                    {online_
                      ? platformText(record) ?? <span className="rk-none" title={t("Plataforma ainda não informada pelo servidor")}>—</span>
                      : record.inProgress ? t("Em andamento") : record.durationMs !== undefined ? formatDuration(record.durationMs) : dash}
                  </span>
                  <span className="rk-when">{when ?? dash}</span>
                  <span className="rk-c">{videoCell(record)}</span>
                </li>
              );
            })}
            {Array.from({ length: emptySlots }, (_, index) => {
              const position = lastPosition + index + 1;
              return (
                <li key={`vago-${position}`} className="rk-row is-vacant" aria-label={t("{position}º lugar vago", { position })}>
                  <span className="rk-pos"><Ordinal n={position} /></span>
                  <span className="rk-initials" aria-hidden="true">_ _ _</span>
                  <span className="rk-r rk-score" aria-hidden="true">· · · · · ·</span>
                  <span className="rk-c" aria-hidden="true">—</span><span className="rk-c" aria-hidden="true">—</span>
                  <span className="rk-c" aria-hidden="true">—</span><span aria-hidden="true">—</span><span className="rk-c" aria-hidden="true">—</span>
                </li>
              );
            })}
          </ol>
          {online_ && online.length > 0 && (
            <footer className="rk-table-foot">
              <span>{onlineRecords.some((record) => record.verified === false) && <><span className="rk-review">*</span> {t("Resultado em revisão pela equipe ShmupX")}</>}</span>
              {onlineRecords.some((record) => !record.platform || !record.video) && <span>{t("Plataforma e vídeo aparecem quando o servidor enviar esses dados")}</span>}
            </footer>
          )}
          {scope === "local" && localRecords.length > 0 && <p className="muted room-ranking-note">{t("Todas as suas runs válidas deste jogo neste PC. Só o seu recorde vai para o ranking online.")}</p>}
        </div>
      </div>

      <aside className="room-ranking-details rk-details" aria-label={t("Detalhes do registro")}>
        <h2>{t("DETALHES DO REGISTRO")}</h2>
        {current ? (() => {
          const { nick, real } = identity(current);
          const handle = handleOf(current.uid);
          const status = statusOf(current);
          const thumbOk = current.video && thumbFailed !== current.video.id;
          const sub = [real ?? (handle ? `@${handle}` : undefined), current.you ? t("Você") : undefined].filter(Boolean).join(" · ");
          const stats: { label: string; value: ReactNode; tone?: string }[] = online_ ? [
            { label: t("Enviado em"), value: current.submittedAt !== undefined ? formatWhen(current.submittedAt, locale) : dash },
            { label: t("Posição"), value: current.position > 0 ? t("{position}º de {total}", { position: current.position, total }) : dash },
            { label: t("Modo"), value: current.mode ? t(MODE_LABEL[current.mode]) : dash },
            { label: t("Continues"), value: current.continues === undefined ? dash : current.continues === 0 ? t("0 · 1CC") : current.continues },
            { label: t("Vidas perdidas"), value: current.livesLost ?? dash },
            { label: t("Runs"), value: current.runs ?? dash },
            { label: t("Nota normalizada"), value: current.rating !== undefined ? current.rating.toFixed(2) : dash },
            { label: t("Status"), value: status.label, tone: status.tone },
            { label: t("Plataforma"), value: platformText(current) ?? dash },
            { label: t("Início da run"), value: current.startedAt !== undefined ? formatWhen(current.startedAt, locale) : dash },
            ...(current.durationMs !== undefined ? [{ label: t("Duração"), value: formatDuration(current.durationMs) }] : []),
          ] : [
            { label: t("Início da run"), value: current.startedAt !== undefined ? formatWhen(current.startedAt, locale) : dash },
            { label: t("Duração"), value: current.inProgress ? t("Em andamento") : current.durationMs !== undefined ? formatDuration(current.durationMs) : dash },
            { label: t("Status"), value: status.label, tone: status.tone },
            { label: t("Placar"), value: score(current.score, locale) },
          ];
          return (
            <>
              <div className={`rk-video${current.video ? "" : " is-empty"}`}>
                <span className="rk-corner is-tl" aria-hidden="true" /><span className="rk-corner is-br" aria-hidden="true" />
                {current.video ? (
                  <button type="button" className="rk-video-play" onClick={() => openYouTube(current.video!.id)}
                    aria-label={current.video.live ? t("Assistir à live no YouTube") : t("Assistir no YouTube")}>
                    {thumbOk && <img src={`https://i.ytimg.com/vi/${current.video.id}/hqdefault.jpg`} alt="" onError={() => setThumbFailed(current.video!.id)} />}
                    <span className="rk-video-btn" aria-hidden="true"><i /></span>
                    {current.video.live && <span className="rk-video-live"><span className="rk-live-dot" aria-hidden="true" />LIVE</span>}
                  </button>
                ) : (
                  <>
                    <span className="rk-video-btn" aria-hidden="true"><i /></span>
                    <b>{t("Sem vídeo do YouTube")}</b>
                    <span>{t("Este registro não tem gravação publicada.")}</span>
                  </>
                )}
              </div>

              <div className="rk-detail-player">
                <img src={avatarOf(profiles, current.uid)} alt="" />
                <div>
                  <strong>{nick}</strong>
                  <small>{sub || "\u00a0"}</small>
                </div>
                {online_ && <span className="rk-detail-rank"><Ordinal n={current.position} /></span>}
              </div>

              <div className="rk-detail-score" aria-label={t("Placar")}>
                <span className="rk-label">{current.mode ? t(MODE_LABEL[current.mode]) : t("Placar")}</span>
                <span className="rk-detail-num">{score(current.score, locale)}</span>
              </div>

              <dl className="rk-stats">
                {stats.map((item) => (
                  <div key={item.label}><dt>{item.label}</dt><dd className={item.tone ? `rk-status is-${item.tone}` : undefined}>{item.value}</dd></div>
                ))}
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

              <div className="rk-actions">
                <button type="button" className="room-action rk-watch" disabled={!current.video} onClick={() => current.video && openYouTube(current.video.id)}>
                  <YouTubeIcon />{current.video?.live ? t("Assistir à live") : t("Assistir no YouTube")}
                </button>
                {!current.video && <p>{t("Disponível quando a run tiver vídeo vinculado")}</p>}
              </div>
            </>
          );
        })() : (
          <p className="muted">{t("Selecione um registro para ver os detalhes.")}</p>
        )}
      </aside>
    </section>
  );
}

function YouTubeIcon() {
  return (
    <svg className="rk-yt-icon" viewBox="0 0 18 13" aria-hidden="true" focusable="false">
      <rect x=".5" y=".5" width="17" height="12" rx="2" />
      <path d="M7 3.8 12 6.5 7 9.2Z" />
    </svg>
  );
}
