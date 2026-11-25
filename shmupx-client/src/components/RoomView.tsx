import { useI18n } from "../i18n";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import ChatFeed from "./ChatFeed";
import ChatInput from "./ChatInput";
import type { ChatMessage, GameRoom } from "../types/room";
import type { SessionRun } from "../types/submission";
import type { ChatStatus } from "../data/useRoomChat";
import { answer as robotxAnswer } from "../data/robotx";
import { useGameArt } from "../data/gameArt";
import { ArcadeCabinet } from "./ArcadeCabinet";
import { useCountdown } from "../data/useLive";
import type { PresenceStatus, RoomPresence } from "../data/useRoomPresence";
import { usePresenceMode } from "../data/presenceMode";
import { useAccountPlaying } from "../data/playing";
import { openWatchWindow } from "./WatchWindow";
import { useGameLauncher } from "./useGameLauncher";
import { formatPlayTime, useGameStats } from "../data/useGameStats";
import { useRoomPlay } from "../data/roomPlay";
import PlaySetup from "./PlaySetup";
import { isSupported } from "../data/validation";
import RankedPanel, { useRankedLive } from "./RankedPanel";
import { avatarOf, usePublicProfiles } from "../data/publicProfile";



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
const ROBOTX = "RobotX";

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

const STATUS_ORDER: Record<PresenceStatus, number> = { live: 0, online: 1, away: 2, offline: 3 };
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
    : run.status === "submitted" ? t("Última run enviada: o resultado entrou em revisão.")
    : run.status === "expired" ? t("O tempo da ficha acabou e a gravação foi encerrada; a gameplay está sendo processada.")
    : t("A última run não foi enviada: {reason}", { reason: run.message ?? t("motivo não informado") });
  return (
    <p className={`room-banner ${tone}`} role="status">
      {text}
      <button type="button" className="text-action" onClick={onOpenRanking}>{t("Ver no ranking")}</button>
    </p>
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
  const { t } = useI18n();
  const presenceMode = usePresenceMode();
  const accountPlaying = useAccountPlaying();
  // Play Training / Play Ranked / Play Live Ranked: modo e plataforma em useGameLauncher.
  // Play Ranked (com ou sem live) e as opções dele: RankedPanel, o mesmo do painel da Coleção.
  const ranked = useRankedLive(room);
  const launcher = useGameLauncher(room, { onLive: (platform) => void ranked.start(platform, launcher.ensureInstalled) });
  const stats = useGameStats(playerId, room.id);
  const [isStopping, setIsStopping] = useState(false);
  const [filter, setFilter] = useState<ChatFilter>("everyone");
  const [sortBy, setSortBy] = useState<SortBy>("score");
  // Respostas do RobotX desta sessão (locais, por sala).
  const [botReplies, setBotReplies] = useState<ChatMessage[]>(() => readRobotxReplies(playerId, room.id));
  const knowledge = room.robotx ?? [];
  const { live, liveHere, liveOn, liveError } = ranked;
  const countdown = useCountdown(live.state.status === "live" ? live.state.ends_at_ms : null);
  const robotxReady = knowledge.length > 0;

  useEffect(() => {
    setIsStopping(false);
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

  async function handleStopGame() {
    launcher.setError(null);
    setIsStopping(true);
    try {
      await invoke("finalizar_corrida_manual");
    } catch (error) {
      launcher.setError(typeof error === "string" ? error : "Falha ao interromper a run manualmente.");
    } finally {
      setIsStopping(false);
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
  const stopping = isStopping || launcher.isStopping;

  // PARAR JOGO: run no PC (encerra a gravação e a análise tira o placar), jogo no MiSTer (volta ao
  // menu; o agente registra o placar) ou treino no PC (fecha o jogo).
  const stopGame = isRecording ? handleStopGame
    : launcher.misterGame ? () => void launcher.stopMister()
    : practicePid !== undefined ? () => void invoke("treino_parar", { pid: practicePid }).catch((error) => launcher.setError(String(error)))
    : null;

  // Jogador conectado sempre primeiro, depois os outros presentes; o RobotX vem no fim e não conta
  // como jogador. Hi-Score da sala ainda não existe: fica "—" em vez de um número inventado.
  // Os outros: online, depois ausentes, depois offline; por nome quando essa é a ordem escolhida.
  const meLive = liveHere && live.state.status === "live";
  // O próprio status segue o escolhido no avatar (Ausente vale em todas as salas), como os outros veem.
  // O jogo aberto na conta (aqui, no MiSTer ou pelo celular) aparece na linha do jogador.
  const myGame = accountPlaying ? { id: accountPlaying.gameId, title: accountPlaying.title, kind: accountPlaying.kind } : undefined;
  const me = { id: "you", name: playerName, score: null as number | null, playing: isRecording || Boolean(myGame), status: (meLive ? "live" : presenceMode === "away" ? "away" : "online") as PresenceStatus, live: undefined as RoomPresence["live"], game: myGame, you: true };
  const rest = others
    .map((o) => ({ id: o.uid, name: o.name, score: null as number | null, playing: o.playing, status: o.status, live: o.live, game: o.game, you: false }))
    .sort((a, b) => STATUS_ORDER[a.status] - STATUS_ORDER[b.status] || (sortBy === "name" ? a.name.localeCompare(b.name) : 0));
  const sorted = [me, ...rest];
  const avatars = usePublicProfiles([playerId ?? "", ...others.map((o) => o.uid)]);
  const players = sorted;

  return (
    <section className="game-room" aria-label={t("Sala de {game}", { game: room.title })}>
      <div className="game-room-column">
        <header className="game-room-header">
          {/* Experimento: o gabinete arcade no lugar do card da arte. */}
          <ArcadeCabinet room={room} className="game-room-cabinet" />
          <div className="game-room-main">
            <div className="game-room-title">
              <div>
                <h1 title={room.title}>{room.title}</h1>
                <dl className="game-room-stats">
                  <div><dt>{t("Fichas gastas")}</dt><dd>{stats ? stats.credits.toLocaleString() : "—"}</dd></div>
                  <div><dt>{t("Tempo de jogo")}</dt><dd>{stats ? formatPlayTime(stats.seconds) : "—"}</dd></div>
                </dl>
              </div>
            </div>
            <PlaySetup play={play} owns={launcher.owns} disabled={gameOpen || liveOn || stopping} />
            {/* Jogar é pela ficha (Training ou Ranked); aqui só o Parar jogo, com um jogo aberto. */}
            {(gameOpen || stopping) && (
              <div className="game-room-actions">
                <button type="button" className="room-action stop stop-action" onClick={stopGame ?? undefined} disabled={stopping || !stopGame}>
                  <span className="action-icon is-square" aria-hidden="true" />{t(stopping ? "Encerrando o jogo…" : "Parar jogo")}
                </button>
              </div>
            )}
            {comingSoon && <p className="room-soon-note"><span className="coming-soon-tag">{t("Coming soon")}</span>{t("Este jogo ainda não tem script de validação: dá para treinar, e Play Ranked e a live ranked chegam quando ele for aprovado.")}</p>}
          </div>
          {/* Destaque à direita: a ficha é o Play Ranked (clique nela); abaixo, as opções da partida. */}
          <RankedPanel room={room} uid={playerId} launcher={launcher} mode={mode} platform={platform} ranked={ranked} busy={gameOpen || stopping} />
        </header>

        {isRecording && <p className="room-banner recording" role="status"><span className="status-dot" /> {t("GRAVANDO RUN")}</p>}
        {liveHere && live.state.status === "live" && (
          <p className="room-banner recording" role="status">
            <span className="status-dot" /> {t("AO VIVO no YouTube")}{countdown && ` · ${t("termina em {time}", { time: countdown })}`}{live.state.public ? ` · ${t("pública")}` : ` · ${t("não listada")}`}
            {live.state.watch_url && <button type="button" className="text-action" onClick={() => void live.open(live.state.watch_url!)}>{t("Abrir no YouTube")}</button>}
          </p>
        )}
        {liveHere && live.state.status === "live" && live.state.silent && <p className="room-banner warning">{t("Não foi possível capturar o som do sistema: a live está sem áudio do jogo.")}</p>}
        {liveHere && (live.state.status === "waiting" || live.state.status === "preparing" || live.state.status === "connecting") && <p className="room-banner" role="status">{live.state.status === "waiting" ? t("Abrindo o jogo; a live começa quando a janela do jogo aparecer.") : live.state.status === "preparing" ? t("Criando a live no seu canal…") : t("Enviando a janela do jogo para o YouTube; a live entra no ar assim que o vídeo chegar.")}</p>}
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

      <aside className="game-room-players" aria-label={t("Jogadores na sala")}>
        <h2>{t("Jogadores na sala ({count})", { count: String(players.length) })}</h2>
        <label className="room-sort">
          <span>{t("Ordenar por")}</span>
          <select value={sortBy} onChange={(event) => setSortBy(event.target.value as SortBy)}>
            <option value="score">{t("Hi-Score")}</option>
            <option value="name">{t("Nome")}</option>
          </select>
        </label>
        <ul>
          {sorted.map((player) => (
            <li key={player.id} className={`room-player ${player.you ? "is-you" : ""} ${player.live ? "is-live" : ""}`}
              title={player.live ? t("Clique duas vezes para assistir à live de {name}", { name: player.name }) : undefined}
              onDoubleClick={player.live ? () => void openWatchWindow(room.id, player.id, player.name, t("{name} · ao vivo · {game}", { name: player.name, game: room.title })) : undefined}>
              <span className="room-player-avatar" aria-hidden="true"><img className="avatar-img" src={avatarOf(avatars, player.you ? playerId : player.id)} alt="" /></span>
              <span className="room-player-info">
                <strong>{player.name}{player.you && ` ${t("(você)")}`}</strong>
                {player.game?.kind === "training"
                  ? <small className="room-player-game">▶ {t("Treinando")}</small>
                  : player.game
                  ? <small className="room-player-game" title={t("Jogando {game}", { game: player.game.title })}>
                      ▶ {player.game.title}
                      {player.game.kind && <span className={`room-player-kind is-${player.game.kind}`}>{t(KIND_LABEL[player.game.kind])}</span>}
                    </small>
                  : <small>{player.score === null ? t("Hi-Score: —") : t("Hi-Score: {score}", { score: player.score.toLocaleString() })}</small>}
              </span>
              <span className="room-player-state">
                {player.status === "live"
                  ? <span className="live-badge" role="img" aria-label={t("Ao vivo")} title={t("Ao vivo")}>LIVE</span>
                  : <>
                    {player.playing && t("JOGANDO")}
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
    </section>
  );
}
