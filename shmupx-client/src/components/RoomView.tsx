import { useI18n } from "../i18n";
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import ChatFeed from "./ChatFeed";
import ChatInput from "./ChatInput";
import type { ChatMessage, GameRoom } from "../types/room";
import { podiumPlaces, samePodium, type SessionRun } from "../types/submission";
import { invoke } from "@tauri-apps/api/core";
import type { ChatStatus } from "../data/useRoomChat";
import { answer as robotxAnswer } from "../data/robotx";
import { useGameArt } from "../data/gameArt";
import { ArcadeCabinet } from "./ArcadeCabinet";
import { useCountdown } from "../data/useLive";
import type { PresenceStatus, RoomPresence } from "../data/useRoomPresence";
import { usePresenceMode } from "../data/presenceMode";
import { useAccountPlaying } from "../data/playing";
import { useRecordGameplay } from "../data/recordGameplay";
import { useMyJoinedAt } from "../data/useRoomPresence";
import { connectionLevel, estimatePing, useNetProfile, type NetProfile } from "../data/connectionQuality";
import { useNetplayPartner } from "../data/netplay";
import { useLastRankedMode } from "../data/gameModes";
import { stopGame as stopPlaying, useStoppingGame } from "../data/stopGame";
import { openWatchWindow } from "./WatchWindow";
import { useGameLauncher } from "./useGameLauncher";
import { formatLastPlayed, formatPlayTime, useGameStats } from "../data/useGameStats";
import { useGameRanking } from "../data/useRanking";
import { useRoomPlay } from "../data/roomPlay";
import PlaySetup from "./PlaySetup";
import { isSupported } from "../data/validation";
import RankedPanel, { useRankedLive } from "./RankedPanel";
import EventsCarousel from "./EventsCarousel";
import { avatarOf, usePublicProfiles } from "../data/publicProfile";
import { netplayCertified } from "../data/netplay";
import { inviteToPlay } from "./NetplayCenter";
import PlayerContextMenu from "./PlayerContextMenu";



export interface RoomViewProps {
  room: GameRoom;
  messages: ChatMessage[];
  /** Estado da conexão com o chat da sala (Firestore). */
  chatStatus?: ChatStatus;
  chatError?: string | null;
  /** uid do jogador conectado, para destacar as mensagens dele. */
  playerId?: string;
  isRecording: boolean;
  /** Nome do jogador conectado, listado entre os presentes na sala. */
  playerName: string;
  onSendMessage: (text: string) => void | Promise<void>;
  /** Abre o ranking deste jogo na mesma aba da sala. */
  onOpenRanking: () => void;
  /** Volta para a Coleção (as abas de cima saíram). */
  onShowCollection?: () => void;
  /** Run mais recente deste jogo nesta sessão: o aviso mostra o andamento e o desfecho. */
  lastRun?: SessionRun;
  /** Motivo de a última tentativa de gravação não ter começado (ex.: FFmpeg, ficha). */
  startError?: string;
  /** Jogo aberto no MODO TREINO nesta sala (processo): o PARAR JOGO o fecha. */
  practicePid?: number;
  /** Outros jogadores presentes na sala agora (presença em tempo real). */
  others?: RoomPresence[];
}

/** Filtros do chat (não confundir com as abas de sala): todos ou só a conversa com o RobotX. */
type ChatFilter = "everyone" | "robotx";
type SortBy = "score" | "name";

/** Modo do jogo aberto, em etiqueta ao lado do título na lista de jogadores. No treino, a lista não
 *  mostra o jogo: só "Treinando". */
const KIND_LABEL = { ranked: "Ranked", live: "Live" } as const;
const MODE_LABEL = { highscore: "Hi-Score", caravan: "Caravan" } as const;
const ROBOTX = "RobotX";

const MEDAL_LABEL = ["", "1º lugar no ranking deste jogo", "2º lugar no ranking deste jogo", "3º lugar no ranking deste jogo"];
/** Medalha de ouro, prata ou bronze: 1º, 2º ou 3º lugar no ranking (Hi-Score) do jogo da sala. */
/** Barrinhas da conexão com o jogador (verde, amarelo ou vermelho); o ping aparece ao passar o mouse. */
function ConnectionBars({ ping }: { ping: number | null }) {
  const { t } = useI18n();
  const level = ping === null ? null : connectionLevel(ping);
  const label = ping === null ? t("Conexão ainda sem medida") : t("Ping: ~{ms} ms", { ms: String(ping) });
  const filled = level === "good" ? 3 : level === "fair" ? 2 : level === "poor" ? 1 : 0;
  return <span className={`connection-bars ${level ? `is-${level}` : "is-unknown"}`} role="img" aria-label={label} title={label}>
    {[1, 2, 3].map((bar) => <i key={bar} className={bar <= filled ? "is-on" : undefined} />)}
  </span>;
}

function Medal({ place }: { place: number | undefined }) {
  const { t } = useI18n();
  if (!place || place > 3) return null;
  const kind = place === 1 ? "gold" : place === 2 ? "silver" : "bronze";
  return <img className={`room-player-medal is-${kind}`} src={`/img/medals/medal-${place}.png`} alt={t(MEDAL_LABEL[place])} title={t(MEDAL_LABEL[place])} draggable={false} />;
}

/** Conversa reservada com o RobotX, por jogador e sala: continua ao sair e voltar à sala. */
const robotxKey = (playerId: string | undefined, roomId: string) => `shmupx.client.robotx.v1.${playerId ?? "anon"}.${roomId}`;
function readRobotxReplies(playerId: string | undefined, roomId: string): ChatMessage[] {
  try {
    const value = JSON.parse(localStorage.getItem(robotxKey(playerId, roomId)) ?? "[]");
    return Array.isArray(value) ? value.filter((m): m is ChatMessage => m && typeof m.id === "string" && typeof m.text === "string" && typeof m.timestamp === "string") : [];
  } catch {
    return [];
  }
}

const STATUS_LABEL: Record<PresenceStatus, string> = { live: "Ao vivo", online: "Online", away: "Ausente", offline: "Offline" };
const MENCAO_ROBOTX = /@robotx\b/i;

export { DEFAULT_COVER } from "../data/gameArt";

/** Capa do jogo (ver `useGameArt`): em preto e branco se o jogo da Steam não está na biblioteca. */
export function GameArt({ room, className = "", decorative = false, children }: { room: GameRoom; className?: string; decorative?: boolean; children?: ReactNode }) {
  const { t } = useI18n();
  const art = useGameArt(room);
  return (
    <div className={`game-art art-${room.id} ${art.kind} ${art.store ? "is-store" : ""} ${art.grayscale ? "is-not-owned" : ""} ${className}`}>
      <img src={art.src} alt={decorative ? "" : t("Capa de {game}", { game: room.title })} loading="lazy" onError={art.onError} />
      {children}
    </div>
  );
}

/** Aviso do andamento/desfecho da última run do jogo. */
function RunOutcome({ run, onOpenRanking }: { run: SessionRun; onOpenRanking: () => void }) {
  const { t } = useI18n();
  if (run.status === "recording") return null;
  const tone = run.status === "submitted" ? "success" : run.status === "failed" ? "warning" : "";
  const text =
    run.status === "analyzing" ? t("Analisando a gameplay da última run. O resultado aparece aqui quando terminar.")
    : run.status === "submitted" ? (run.online === false
      ? t("Run salva no seu ranking local. Ela ficou abaixo do seu recorde, que continua no ranking online.")
      : t("Novo recorde! A run entrou no ranking online (em revisão) e no seu ranking local."))
    : run.status === "expired" ? t("O tempo da ficha acabou e a partida foi encerrada; o resultado está sendo processado.")
    : t("A última run não foi enviada: {reason}", { reason: run.message ?? t("motivo não informado") });
  const highlight = run.highlight;
  return (
    <>
      <p className={`room-banner ${tone}`} role="status">
        {text}
        {run.status === "submitted" && run.position !== undefined && run.position <= 3 && ` ${t("Você está em {place}º lugar no ranking!", { place: run.position })}`}
        <button type="button" className="text-action" onClick={onOpenRanking}>{t("Ver no ranking")}</button>
      </p>
      {/* Ranked com live no top 3: a live vai para o canal oficial da ShmupX. */}
      {highlight && (
        <p className={`room-banner ${highlight.status === "posted" ? "success" : highlight.status === "failed" ? "warning" : ""}`} role="status">
          {highlight.status === "uploading"
            ? t("Sua live vai para o canal oficial da ShmupX, com seu nome, placar e posição. Enviando o vídeo… {percent}%", { percent: Math.round((highlight.progress ?? 0) * 100) })
            : highlight.status === "posted" ? t("Sua live está no canal oficial da ShmupX!")
            : t("Não foi possível publicar sua live no canal da ShmupX agora. Ela continua no seu canal do YouTube.")}
          {highlight.status === "posted" && highlight.videoUrl && (
            <button type="button" className="text-action" onClick={() => void invoke("live_abrir", { url: highlight.videoUrl })}>{t("Assistir")}</button>
          )}
        </p>
      )}
    </>
  );
}

/** Ícone de jogadores (filtro "Para todos"). */
const IconPlayers = () => (
  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true">
    <circle cx="9" cy="8" r="3.2" /><path d="M3 19c.6-3.2 3-5 6-5s5.4 1.8 6 5" /><circle cx="17" cy="9" r="2.4" /><path d="M16 14.2c2.6.1 4.4 1.7 5 4.3" />
  </svg>
);

/** Ícone do RobotX (filtro e avatar). */
const IconRobot = ({ size = 15 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
    <rect x="4" y="7" width="16" height="12" rx="3" fill="#c58bff" /><rect x="7.5" y="10.5" width="3" height="3" rx="1" fill="#1f1f1f" /><rect x="13.5" y="10.5" width="3" height="3" rx="1" fill="#1f1f1f" />
    <rect x="9" y="15.5" width="6" height="1.6" rx=".8" fill="#1f1f1f" /><rect x="11.2" y="3.5" width="1.6" height="3.5" fill="#ff6b76" /><circle cx="12" cy="3.2" r="1.5" fill="#ff6b76" />
  </svg>
);

/** Sala dedicada do jogo: cabeçalho compacto, chat no centro e HUD de jogadores à direita. */
export default function RoomView({ room, messages, chatStatus, chatError, playerId, isRecording, playerName, onSendMessage, onOpenRanking, lastRun, startError, practicePid, others = [] }: RoomViewProps) {
  const { t, locale } = useI18n();
  const presenceMode = usePresenceMode();
  const accountPlaying = useAccountPlaying();
  // "GRAVANDO RUN" só com "Gravar gameplay" ligado; desligado, nenhum vídeo fica guardado.
  const recordGameplay = useRecordGameplay();
  // Play Training / Play Ranked / Play Live Ranked: modo e plataforma em useGameLauncher.
  // Play Ranked (com ou sem live) e as opções dele: RankedPanel, o mesmo do painel da Coleção.
  const ranked = useRankedLive(room);
  const launcher = useGameLauncher(room, { onLive: (platform, mode) => void ranked.start(platform, launcher.ensureInstalled, mode) });
  const stats = useGameStats(playerId, room.id);
  const myJoinedAt = useMyJoinedAt(room.id);
  // Pódio do jogo da sala (Hi-Score): medalha de ouro, prata e bronze e o topo da lista de jogadores.
  // Ao fim de uma ranked, o backend devolve o pódio já com a run: se a ordem for outra, a sala
  // reordena na hora (a rota pública do ranking guarda 60 s em cache, e um pódio mais antigo que o
  // mostrado é ignorado). A cada minuto a sala confere o ranking de novo, para as runs dos outros.
  // Ranking online (Hi-Score) da sala: o pódio (medalhas e ordem da lista) e a posição de cada
  // jogador, à direita na linha dele.
  const podiumRanking = useGameRanking(room.id, "highscore", 100);
  const [podiumState, setPodiumState] = useState<{ roomId: string; places: Map<string, number>; at: number }>(() => ({ roomId: room.id, places: new Map(), at: 0 }));
  const keepPodium = useCallback((places: Map<string, number>, at: number) => setPodiumState((current) => {
    if (current.roomId !== room.id) return { roomId: room.id, places, at };
    if (at < current.at || samePodium(current.places, places)) return current;
    return { roomId: room.id, places, at };
  }), [room.id]);
  useEffect(() => {
    const data = podiumRanking.data;
    if (data && data.jogo_id === room.id) keepPodium(podiumPlaces(data.entradas), Date.parse(data.atualizado_em) || 0);
  }, [podiumRanking.data, room.id, keepPodium]);
  useEffect(() => {
    if (lastRun?.status !== "submitted" || !lastRun.podium || lastRun.gameId !== room.id) return;
    // O pódio chegou agora, calculado pelo backend no envio: vale mais que o ranking já mostrado.
    keepPodium(podiumPlaces(lastRun.podium), Date.now());
  }, [lastRun?.status, lastRun?.podium, lastRun?.gameId, room.id, keepPodium]);
  const reloadPodium = podiumRanking.reload;
  useEffect(() => {
    const timer = window.setInterval(reloadPodium, 60_000);
    return () => window.clearInterval(timer);
  }, [reloadPodium]);
  const podium = podiumState.roomId === room.id ? podiumState.places : new Map<string, number>();
  const rankPositions = new Map<string, number>(podiumRanking.data?.jogo_id === room.id ? podiumRanking.data.entradas.map((e) => [e.jogador_id, e.posicao] as const) : []);
  const [filter, setFilter] = useState<ChatFilter>("everyone");
  const [sortBy, setSortBy] = useState<SortBy>("score");
  // Respostas do RobotX desta sessão (locais, por sala).
  const [botReplies, setBotReplies] = useState<ChatMessage[]>(() => readRobotxReplies(playerId, room.id));
  const knowledge = room.robotx ?? [];
  const { live, liveHere, liveOn, liveError } = ranked;
  const countdown = useCountdown(live.state.status === "live" ? live.state.ends_at_ms : null);
  const robotxReady = knowledge.length > 0;

  useEffect(() => {
    setBotReplies(readRobotxReplies(playerId, room.id));
    setFilter("everyone");
  }, [room.id, playerId]);

  function addBotReply(message: ChatMessage) {
    setBotReplies((current) => {
      const next = [...current, message].slice(-50);
      try { localStorage.setItem(robotxKey(playerId, room.id), JSON.stringify(next)); } catch { /* fica só nesta sessão */ }
      return next;
    });
  }

  // "Só RobotX": mensagens do RobotX e as que o mencionam. É um filtro de leitura; o chat
  // continua o mesmo para todos.
  // O mesmo chat em duas visões: "Para todos" mostra tudo; "Só RobotX" mostra só o RobotX
  // (o que ele disse na sala e o que respondeu reservadamente a você), sem o resto da conversa.
  const visible = useMemo(() => {
    const all = [...messages, ...botReplies].sort((a, b) => a.timestamp.localeCompare(b.timestamp));
    return filter === "everyone" ? all : all.filter((m) => m.local && m.author === ROBOTX);
  }, [filter, messages, botReplies]);

  function reply(question: string) {
    const result = robotxAnswer(question, knowledge);
    const asked = question.replace(MENCAO_ROBOTX, "").trim();
    // Um instante depois da pergunta, para a resposta aparecer abaixo dela.
    window.setTimeout(() => {
      addBotReply({ id: `robotx-${Date.now()}`, author: ROBOTX, text: result.text, timestamp: new Date().toISOString(), local: true, inReplyTo: asked });
    }, 350);
  }

  async function send(text: string) {
    // "Só RobotX": a pergunta vai só para o RobotX, reservada; nada é publicado na sala.
    if (filter === "robotx") {
      if (robotxReady) reply(text);
      return;
    }
    // "Para todos": vai para a sala; se mencionar @RobotX, ele responde reservadamente.
    await onSendMessage(text);
    if (robotxReady && MENCAO_ROBOTX.test(text)) reply(text);
  }

  // PARAR JOGO: o mesmo que o STOP do topo (data/stopGame).
  async function handleStopGame() {
    launcher.setError(null);
    try {
      await stopPlaying({ uid: playerId, recording: isRecording, practicePid });
    } catch (error) {
      launcher.setError(typeof error === "string" ? error : t("Não foi possível parar o jogo."));
    }
  }

  // Modo de jogo e plataforma: escolhidos no cabeçalho, antes dos botões de Play.
  const play = useRoomPlay(room, launcher.platforms, launcher.owns);
  const { mode, platform } = play;
  // Coming soon (sem script de validação): só treino; ranked e live ranked esperam o script.
  const comingSoon = !isSupported(room);

  // INICIAR JOGO vira PARAR JOGO (vermelho) com o jogo aberto: run no PC (encerra a gravação e
  // a análise tira o placar), jogo no MiSTer (volta ao menu; o agente registra o placar) ou
  // treino no PC (fecha o jogo).
  const gameOpen = isRecording || practicePid !== undefined || launcher.misterGame !== null;
  const stopping = useStoppingGame() || launcher.isStopping;

  // PARAR JOGO: run no PC (encerra a gravação e a análise tira o placar), jogo no MiSTer (volta ao
  // menu; o agente registra o placar) ou treino no PC (fecha o jogo).
  const stopGame = gameOpen ? () => void handleStopGame() : null;

  // Lista de jogadores (com o próprio jogador): pódio do ranking e depois a ordem de chegada; o
  // RobotX vem no fim e não conta como jogador.
  const meLive = liveHere && live.state.status === "live";
  // O próprio status segue o escolhido no avatar (Ausente vale em todas as salas), como os outros veem.
  // O jogo aberto na conta (aqui, no MiSTer ou pelo celular) aparece na linha do jogador.
  const netplayPartner = useNetplayPartner();
  const myLastMode = useLastRankedMode(accountPlaying?.gameId ?? null);
  const myGame = accountPlaying ? { id: accountPlaying.gameId, title: accountPlaying.title, kind: accountPlaying.kind,
    with: netplayPartner?.gameId === accountPlaying.gameId ? netplayPartner.name : undefined,
    mode: accountPlaying.kind !== "training" && myLastMode ? (myLastMode === "caravan" ? "caravan" as const : "highscore" as const) : undefined } : undefined;
  const me = { id: "you", name: playerName, score: null as number | null, playing: isRecording || Boolean(myGame), status: (meLive ? "live" : presenceMode === "invisible" ? "offline" : presenceMode === "away" ? "away" : "online") as PresenceStatus, live: undefined as RoomPresence["live"], game: myGame, you: true, net: undefined as NetProfile | undefined };
  // Conexão com cada jogador (barrinhas): ping estimado pelos pontos de referência dos dois.
  const myNet = useNetProfile();
  const rest = others
    .map((o) => ({ id: o.uid, name: o.name, score: null as number | null, playing: o.playing, status: o.status, live: o.live, game: o.game, you: false, joinedAt: o.joinedAt, net: o.net as NetProfile | undefined }));
  // Ranking: 1º, 2º e 3º do ranking do jogo da sala primeiro; os outros na ordem em que chegaram à
  // sala (quem não tem a chegada gravada, de um Client antigo, vai para o fim). Nome: alfabética.
  const place = (p: { id: string; you: boolean }) => podium.get(p.you ? playerId ?? "" : p.id) ?? 99;
  const arrival = (p: { joinedAt?: number }) => p.joinedAt ?? Number.MAX_SAFE_INTEGER;
  const sorted = [{ ...me, joinedAt: myJoinedAt }, ...rest].sort((a, b) =>
    sortBy === "name" ? a.name.localeCompare(b.name) : place(a) - place(b) || arrival(a) - arrival(b) || a.name.localeCompare(b.name));
  const avatars = usePublicProfiles([playerId ?? "", ...others.map((o) => o.uid)]);
  const players = sorted;

  // Clique direito num jogador › Convidar para jogar (treino em dupla online no ShmupX Emulator).
  const [playerMenu, setPlayerMenu] = useState<{ x: number; y: number; player: (typeof sorted)[number] } | null>(null);
  const closePlayerMenu = useCallback(() => setPlayerMenu(null), []);
  const inviteBlocked = (player: (typeof sorted)[number]): string | null =>
    !netplayCertified(room) ? t("{game} ainda não tem treino em dupla", { game: room.title })
    : !launcher.owns("emulator") ? t("Você precisa ter {game} no ShmupX Emulator", { game: room.title })
    : player.status === "offline" ? t("{name} está offline", { name: player.name })
    : player.game || player.playing ? t("{name} já está jogando", { name: player.name })
    : gameOpen ? t("Feche o seu jogo antes de convidar")
    : null;

  return (
    <section className="game-room" aria-label={t("Sala de {game}", { game: room.title })}>
      <div className="game-room-column">
        <header className="game-room-header has-events">
          {/* O gabinete arcade no lugar do card da arte; como em Minhas coleções, o painel mexe com o
              controle e o clique abre os controles do emulador. O nome do jogo já está no menu do topo. */}
          <ArcadeCabinet room={room} className="game-room-cabinet" controls />
          <div className="game-room-main">
            <div className="game-room-title">
              <div>
                <dl className="game-room-stats">
                  <div><dt>{t("Total de fichas gastas")}</dt><dd>{stats ? stats.credits.toLocaleString() : "—"}</dd></div>
                  <div><dt>{t("Tempo de jogo")}</dt><dd>{stats ? formatPlayTime(stats.seconds) : "—"}</dd></div>
                  <div><dt>{t("Último ranked")}</dt><dd>{stats?.lastAt ? formatLastPlayed(stats.lastAt, locale) : "—"}</dd></div>
                </dl>
              </div>
            </div>
            {/* Jogar é pela ficha (Training ou Ranked); aqui só o Parar jogo, com um jogo aberto. */}
            {(gameOpen || stopping) && (
              <div className="game-room-actions">
                <button type="button" className="room-action stop stop-action" onClick={stopGame ?? undefined} disabled={stopping || !stopGame}>
                  <span className="action-icon is-square" aria-hidden="true" />{t(stopping ? "Parando…" : "Parar jogo")}
                </button>
              </div>
            )}
            {comingSoon && <p className="room-soon-note"><span className="coming-soon-tag">{t("Em breve")}</span>{t("Por enquanto, só treino. O Ranked e a live ranked chegam quando o jogo for aprovado.")}</p>}
          </div>
          {/* Destaque à direita: a ficha é o Play Ranked (clique nela); abaixo, as opções da partida. */}
          <RankedPanel room={room} uid={playerId} launcher={launcher} mode={mode} platform={platform} ranked={ranked} busy={stopping}
            setup={<PlaySetup play={play} owns={launcher.owns} disabled={gameOpen || liveOn || stopping} compact />} />
          {/* O espaço que sobra à direita: os eventos do Lab, com PARTICIPAR. */}
          <EventsCarousel room={room} uid={playerId} />
        </header>

        {isRecording && <p className="room-banner recording" role="status"><span className="status-dot" /> {t(recordGameplay ? "GRAVANDO RUN" : "RUN RANKED")}</p>}
        {liveHere && live.state.status === "live" && live.state.game_open && (
          <p className="room-banner recording" role="status">
            <span className="status-dot" /> {t("AO VIVO no YouTube")}{countdown && ` · ${t("termina em {time}", { time: countdown })}`}{live.state.public ? ` · ${t("pública")}` : ` · ${t("não listada")}`}
            {live.state.watch_url && <button type="button" className="text-action" onClick={() => void live.open(live.state.watch_url!)}>{t("Abrir no YouTube")}</button>}
          </p>
        )}
        {liveHere && live.state.status === "live" && live.state.game_open && live.state.silent && <p className="room-banner warning">{t("Não foi possível capturar o som do sistema: a live está sem áudio do jogo.")}</p>}
        {/* Live Ranked: primeiro a live entra no ar; só então o jogo abre no modo ranked. */}
        {liveHere && (live.state.status === "preparing" || live.state.status === "connecting" || (live.state.status === "live" && !live.state.game_open)) && (
          <p className="room-banner" role="status">
            {live.state.status === "preparing" ? t("Criando a live no seu canal…")
              : live.state.status === "connecting" ? t("Conectando a live ao YouTube. O jogo abre assim que ela entrar no ar.")
              : t("Live no ar! Abrindo o jogo no Ranked (1 ficha, sem continue)…")}
          </p>
        )}
        {liveHere && live.state.status === "failed" && live.state.error && <p className="room-banner warning" role="alert">{t("A live não continuou: {reason}", { reason: t(live.state.error) })}</p>}
        {liveHere && live.state.status === "idle" && live.state.ended_by_time_limit && <p className="room-banner">{t("A live terminou no limite de 45 minutos.")}</p>}
        {liveError && <p className="room-banner warning" role="alert">{t(liveError)}</p>}
        {launcher.banners}
        {launcher.dialogs}
        {startError && <p className="room-banner warning" role="alert">{t("A gravação não começou: {reason}", { reason: startError })}</p>}
        {lastRun && <RunOutcome run={lastRun} onOpenRanking={onOpenRanking} />}

        <div className="room-chat-panel">
          <div className="room-chat-filters" role="tablist" aria-label={t("Filtro do chat")}>
            <button type="button" role="tab" aria-selected={filter === "everyone"} onClick={() => setFilter("everyone")}>
              <IconPlayers /> {t("Para todos")}
            </button>
            <button type="button" role="tab" aria-selected={filter === "robotx"} onClick={() => setFilter("robotx")} title={robotxReady ? t("Pergunte ao RobotX sobre este jogo") : t("O RobotX ainda não chegou a esta sala.")}>
              <IconRobot /> {t("Só RobotX")}{!robotxReady && <small className="robotx-soon">{t("em breve")}</small>}
            </button>
          </div>
          <ChatFeed
            messages={visible}
            status={filter === "robotx" ? "ready" : chatStatus}
            error={chatStatus === "error" && filter === "everyone" ? chatError : null}
            playerId={playerId}
            roomId={room.id}
            emptyText={filter === "robotx" ? (robotxReady ? t("Pergunte ao RobotX sobre {game}: controles, naves, chefes, dicas…", { game: room.title }) : t("O RobotX ainda não chegou a esta sala.")) : undefined}
          />
        </div>
        <ChatInput
          onSendMessage={send}
          disabled={filter === "robotx" ? !robotxReady : chatStatus === "error"}
          error={chatStatus === "error" || filter === "robotx" ? null : chatError}
          placeholder={filter === "robotx" ? (robotxReady ? t("Pergunta reservada para o RobotX") : t("O RobotX ainda não chegou a esta sala.")) : t("Mensagem para a sala")}
        />
      </div>

      <aside className="game-room-players" aria-label={t("Jogadores na sala")} data-tutorial="room-players">
        <h2>{t("Jogadores na sala ({count})", { count: String(players.length) })}</h2>
        <label className="room-sort">
          <span>{t("Ordenar por")}</span>
          <select value={sortBy} onChange={(event) => setSortBy(event.target.value as SortBy)}>
            <option value="score">{t("Ranking")}</option>
            <option value="name">{t("Nome")}</option>
          </select>
        </label>
        <ul>
          {sorted.map((player) => (
            <li key={player.id} className={`room-player ${player.you ? "is-you" : ""} ${player.live ? "is-live" : ""} ${playerMenu?.player.id === player.id ? "is-menu-open" : ""}`}
              title={player.live ? t("Clique duas vezes para assistir à live de {name}", { name: player.name }) : undefined}
              onContextMenu={player.you ? undefined : (event) => { event.preventDefault(); setPlayerMenu({ x: event.clientX, y: event.clientY, player }); }}
              onDoubleClick={player.live ? () => void openWatchWindow(room.id, player.id, player.name, t("{name} · ao vivo · {game}", { name: player.name, game: room.title })) : undefined}>
              <span className="room-player-avatar" aria-hidden="true"><img className="avatar-img" src={avatarOf(avatars, player.you ? playerId : player.id)} alt="" /></span>
              <span className="room-player-info">
                <strong>
                  {player.name}{player.you && ` ${t("(você)")}`}
                  <Medal place={podium.get(player.you ? playerId ?? "" : player.id)} />
                </strong>
                {player.game?.with
                  ? <small className="room-player-game" title={t("Jogando {game} com {name}", { game: player.game.title, name: player.game.with })}>▶ {t("Jogando {game} com {name}", { game: player.game.title, name: player.game.with })}</small>
                  : player.game?.kind === "training"
                  ? <small className="room-player-game" title={t("Treinando {game}", { game: player.game.title })}>▶ {t("Treinando {game}", { game: player.game.title })}</small>
                  : player.game
                  ? <small className="room-player-game" title={t("Jogando {game}", { game: player.game.title })}>
                      ▶ {t("Jogando {game}", { game: player.game.title })}
                      {player.game.kind && <span className={`room-player-kind is-${player.game.kind}`}>{t(KIND_LABEL[player.game.kind])}{player.game.mode && ` ${MODE_LABEL[player.game.mode]}`}</span>}
                    </small>
                  // Sem jogo aberto: online, "Disponível para jogar" (verde); ausente ou offline, o status.
                  : player.status === "away" || player.status === "offline"
                  ? <small className={`room-player-idle is-${player.status}`}>{t(STATUS_LABEL[player.status])}</small>
                  : <small className="room-player-idle is-available">{t("Disponível para jogar")}</small>}
              </span>
              <span className="room-player-state">
                {!player.you && player.status !== "offline" && <ConnectionBars ping={estimatePing(myNet, player.net)} />}
                {player.status === "live"
                  ? <span className="live-badge" role="img" aria-label={t("Ao vivo")} title={t("Ao vivo")}>LIVE</span>
                  : <>
                    {(() => {
                      const position = podium.get(player.you ? playerId ?? "" : player.id) ?? rankPositions.get(player.you ? playerId ?? "" : player.id);
                      return position ? <span className="room-player-rank" title={t("{n}º no ranking online deste jogo (Hi-Score)", { n: String(position) })}>#{position}</span> : null;
                    })()}
                    <span className={`presence-dot is-${player.status}`} role="img" aria-label={t(STATUS_LABEL[player.status])} title={t(STATUS_LABEL[player.status])} />
                  </>}
              </span>
            </li>
          ))}
          <li className={`room-player is-bot ${robotxReady ? "" : "is-soon"}`} title={robotxReady ? t("Mencione @RobotX no chat para perguntar sobre o jogo.") : t("O RobotX ainda não chegou a esta sala.")}>
            <span className="room-player-avatar bot" aria-hidden="true"><IconRobot size={20} /></span>
            <span className="room-player-info"><strong>RobotX</strong>{robotxReady && <small>{t("Pergunte com @RobotX")}</small>}</span>
            <span className="room-player-state bot">{robotxReady ? <><IconRobot size={11} /> {t("BOT")}</> : t("EM BREVE")}</span>
          </li>
        </ul>
      </aside>
      {playerMenu && <PlayerContextMenu x={playerMenu.x} y={playerMenu.y} title={playerMenu.player.name} onClose={closePlayerMenu} items={[{
        label: t("Convidar para jogar"),
        disabledReason: inviteBlocked(playerMenu.player),
        onSelect: () => inviteToPlay({ uid: playerMenu.player.id, name: playerMenu.player.name }, room),
      }]} />}
    </section>
  );
}
