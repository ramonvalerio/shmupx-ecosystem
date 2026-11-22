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
import { liveActive, useCountdown, useLive, useYouTube } from "../data/useLive";
import type { PresenceStatus, RoomPresence } from "../data/useRoomPresence";
import { readEmulatorLibrary, romsFor } from "../data/emulatorLibrary";
import { openWatchWindow } from "./WatchWindow";
import { PLAY_ON, stillInstalled, usePlatformOwnership } from "../data/platforms";
import { useDialogFocus } from "./useDialogFocus";
import type { PlayOn } from "../types/room";

/** Plataformas em que o jogo roda fora do PC: o Client não tem como abri-lo. */
const OWN_HARDWARE: Record<string, string> = { mister: "MiSTer", xbox360: "Xbox 360" };

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
  /** Outros jogadores presentes na sala agora (presença em tempo real). */
  others?: RoomPresence[];
}

/** Filtros do chat (não confundir com as abas de sala): todos ou só a conversa com o RobotX. */
type ChatFilter = "everyone" | "robotx";
type SortBy = "score" | "name";

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
    <div className={`game-art art-${room.id} ${art.kind} ${art.grayscale ? "is-not-owned" : ""} ${className}`}>
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
export default function RoomView({ room, messages, chatStatus, chatError, playerId, isRecording, playerName, onSendMessage, onOpenRanking, onShowCollection, lastRun, startError, others = [] }: RoomViewProps) {
  const { t } = useI18n();
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [isLaunching, setIsLaunching] = useState(false);
  const [launchNotice, setLaunchNotice] = useState<string | null>(null);
  /** Plataformas para escolher ao iniciar (o jogador tem o jogo em mais de uma). */
  const [choosing, setChoosing] = useState<PlayOn[] | null>(null);
  const chooseDialogRef = useDialogFocus(choosing !== null, () => setChoosing(null));
  /** Jogo desinstalado (ou sem o ROM) na plataforma escolhida. */
  const [notFound, setNotFound] = useState<PlayOn | null>(null);
  const notFoundDialogRef = useDialogFocus(notFound !== null, () => setNotFound(null));
  const playable = usePlatformOwnership();
  const [isStopping, setIsStopping] = useState(false);
  const [filter, setFilter] = useState<ChatFilter>("everyone");
  const [sortBy, setSortBy] = useState<SortBy>("score");
  // Respostas do RobotX desta sessão (locais, por sala).
  const [botReplies, setBotReplies] = useState<ChatMessage[]>(() => readRobotxReplies(playerId, room.id));
  const knowledge = room.robotx ?? [];
  const live = useLive();
  const youtube = useYouTube();
  const [publicLive, setPublicLive] = useState(false);
  const [liveError, setLiveError] = useState<string | null>(null);
  const countdown = useCountdown(live.state.status === "live" ? live.state.ends_at_ms : null);
  const liveHere = live.state.game_id === room.id;
  const liveOn = liveActive(live.state);
  const robotxReady = knowledge.length > 0;

  useEffect(() => {
    setLaunchError(null);
    setLaunchNotice(null);
    setChoosing(null);
    setNotFound(null);
    setIsLaunching(false);
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

  /**
   * Plataformas por onde dá para iniciar: as em que o jogador tem o jogo (bibliotecas importadas).
   * Sem nenhuma importada, o que o Client sabe abrir (Steam pelo app_id, emulador pelo ROM).
   */
  function startOptions(): PlayOn[] {
    const owned = playable.tags(room).filter((tag) => tag.owned).map((tag) => tag.id);
    if (owned.length > 0) return owned;
    const known: PlayOn[] = [];
    if (room.steamAppId > 0) known.push("steam");
    if (romsFor(room).length > 0) known.push("emulator");
    return known;
  }

  /** INICIAR JOGO: uma plataforma inicia direto; mais de uma pergunta por qual. */
  function handleStartGame() {
    setLaunchError(null);
    setLaunchNotice(null);
    const options = startOptions();
    if (options.length === 0) {
      setLaunchError(room.platform && OWN_HARDWARE[room.platform]
        ? t("Este jogo roda no {platform}: abra-o no próprio aparelho.", { platform: OWN_HARDWARE[room.platform] })
        : t("O Client ainda não sabe abrir este jogo. Importe o jogo de uma plataforma em Importar jogos."));
      return;
    }
    if (options.length > 1) { setChoosing(options); return; }
    void launchOn(options[0]);
  }

  async function launchOn(platform: PlayOn) {
    setChoosing(null);
    setLaunchError(null);
    setLaunchNotice(null);
    if (platform === "mister") {
      // O MiSTer é outro aparelho: a partida começa nele (a integração ShmupX do MiSTer grava a run).
      setLaunchNotice(t("Inicie {game} no MiSTer. A partida entra no seu perfil pela integração ShmupX do MiSTer.", { game: room.title }));
      return;
    }
    setIsLaunching(true);
    try {
      if (!(await stillInstalled(room, platform))) { setNotFound(platform); return; }
      if (platform === "steam") {
        if (room.steamAppId === 0) throw "Este jogo não tem app_id da Steam no catálogo.";
        await invoke("inicializar_jogo_steam", { steamAppId: room.steamAppId });
      } else {
        await invoke("inicializar_jogo_emulador", { jogoId: room.id, roms: romsFor(room), pasta: readEmulatorLibrary()?.root || null });
      }
    } catch (error) {
      setLaunchError(typeof error === "string" ? error : "Falha ao iniciar o jogo.");
    } finally {
      setIsLaunching(false);
    }
  }

  async function handleStopGame() {
    setLaunchError(null);
    setIsStopping(true);
    try {
      await invoke("finalizar_corrida_manual");
    } catch (error) {
      setLaunchError(typeof error === "string" ? error : "Falha ao interromper a run manualmente.");
    } finally {
      setIsStopping(false);
    }
  }

  async function toggleLive() {
    setLiveError(null);
    try {
      if (liveOn) await live.stop();
      else await live.start(room.id, `${room.title} · ShmupX`, publicLive, room.steamAppId);
    } catch (error) {
      setLiveError(error instanceof Error ? error.message : String(error));
    }
  }
  const youtubeConnected = Boolean(youtube.connection?.connected);
  const liveBlocked = !liveOn && (!youtubeConnected || isRecording || (live.state.status !== "idle" && live.state.status !== "failed"));
  const liveTitle = !youtubeConnected
    ? t("Conecte seu canal em Configurações › Integrações para transmitir.")
    : isRecording ? t("Termine a gravação da run antes de começar uma live.")
    : liveOn && !liveHere ? t("Há uma live em andamento em outra sala.") : undefined;
  const liveLabel = live.state.status === "waiting" ? t("Abrindo o jogo…")
    : live.state.status === "preparing" ? t("Criando a live…")
    : live.state.status === "connecting" ? t("Conectando ao YouTube…")
    : live.state.status === "ending" ? t("Encerrando…")
    : liveOn ? t("End live") : t("Start live");

  const tip = t("Dica: para uma captura estável, jogue em janela ou em tela cheia sem bordas. A tela cheia exclusiva pode não ser capturada. A gravação começa quando a run é autorizada.");

  let mainLabel = "Iniciar jogo";
  let mainAction = handleStartGame;
  let mainDisabled = isLaunching;
  let mainVariant = "primary";
  if (isStopping) {
    mainLabel = "Encerrando a gravação...";
    mainDisabled = true;
    mainVariant = "";
  } else if (isRecording) {
    mainLabel = "[ STOP ]";
    mainAction = handleStopGame;
    mainDisabled = false;
    mainVariant = "stop";
  } else if (isLaunching) {
    mainLabel = "Abrindo o jogo…";
    mainVariant = "";
  }

  // Jogador conectado sempre primeiro, depois os outros presentes; o RobotX vem no fim e não conta
  // como jogador. Hi-Score da sala ainda não existe: fica "—" em vez de um número inventado.
  // Os outros: online, depois ausentes, depois offline; por nome quando essa é a ordem escolhida.
  const meLive = liveHere && live.state.status === "live";
  const me = { id: "you", name: playerName, score: null as number | null, playing: isRecording, status: (meLive ? "live" : "online") as PresenceStatus, live: undefined as RoomPresence["live"], you: true };
  const rest = others
    .map((o) => ({ id: o.uid, name: o.name, score: null as number | null, playing: o.playing, status: o.status, live: o.live, you: false }))
    .sort((a, b) => STATUS_ORDER[a.status] - STATUS_ORDER[b.status] || (sortBy === "name" ? a.name.localeCompare(b.name) : 0));
  const sorted = [me, ...rest];
  const players = sorted;

  return (
    <section className="game-room" aria-label={t("Sala de {game}", { game: room.title })}>
      <div className="game-room-column">
        <header className="game-room-header">
          {/* Experimento: o gabinete arcade no lugar do card da arte. */}
          <ArcadeCabinet room={room} className="game-room-cabinet" />
          <div className="game-room-main">
            <div className="game-room-title">
              {onShowCollection && <button type="button" className="game-room-back" onClick={onShowCollection}>{t("← Minhas coleções")}</button>}
              <h1 title={room.title}>{room.title}</h1>
              <p>{t("Chat e presença em tempo real")}</p>
            </div>
            <div className="game-room-actions">
              <button type="button" className="room-action" onClick={onOpenRanking}>{t("Ranking")}</button>
              <button type="button" className={`room-action ${mainVariant}`} onClick={mainAction} disabled={mainDisabled || liveOn}>{t(mainLabel)}</button>
              <button type="button" className={`room-action live-action ${liveOn ? "is-on" : ""}`} onClick={() => void toggleLive()} disabled={liveBlocked || live.state.status === "ending" || (liveOn && !liveHere)} title={liveTitle}>
                <span aria-hidden="true">{liveOn ? "■" : "▶"}</span> {liveLabel}
              </button>
              <label className="live-public" title={t("Desmarcada, a live é não listada: só quem tem o link assiste.")}>
                <input type="checkbox" checked={liveOn ? live.state.public : publicLive} disabled={liveOn} onChange={(event) => setPublicLive(event.target.checked)} />
                {t("Live pública")}
              </label>
              <span className="game-room-tip" tabIndex={0} data-tip={tip} aria-label={tip}>ⓘ {t("Dica de captura")}</span>
            </div>
          </div>
        </header>

        {isRecording && <p className="room-banner recording" role="status"><span className="status-dot" /> {t("GRAVANDO RUN")}</p>}
        {liveHere && live.state.status === "live" && (
          <p className="room-banner recording" role="status">
            <span className="status-dot" /> {t("AO VIVO no YouTube")}{countdown && ` · ${t("termina em {time}", { time: countdown })}`}{live.state.public ? ` · ${t("pública")}` : ` · ${t("não listada")}`}
            {live.state.watch_url && <button type="button" className="text-action" onClick={() => void live.open(live.state.watch_url!)}>{t("Abrir no YouTube")}</button>}
          </p>
        )}
        {liveHere && live.state.status === "live" && live.state.silent && <p className="room-banner warning">{t("Não foi possível capturar o som do sistema: a live está sem áudio do jogo.")}</p>}
        {liveHere && (live.state.status === "waiting" || live.state.status === "preparing" || live.state.status === "connecting") && <p className="room-banner" role="status">{live.state.status === "waiting" ? t("Abrindo o jogo pela Steam; a live começa quando a janela do jogo aparecer.") : live.state.status === "preparing" ? t("Criando a live no seu canal…") : t("Enviando a janela do jogo para o YouTube; a live entra no ar assim que o vídeo chegar.")}</p>}
        {liveHere && live.state.status === "failed" && live.state.error && <p className="room-banner warning" role="alert">{t("A live não continuou: {reason}", { reason: t(live.state.error) })}</p>}
        {liveHere && live.state.status === "idle" && live.state.ended_by_time_limit && <p className="room-banner">{t("A live terminou no limite de 45 minutos.")}</p>}
        {liveError && <p className="room-banner warning" role="alert">{t(liveError)}</p>}
        {launchError && <p className="room-banner warning" role="alert">{t(launchError)}</p>}
        {launchNotice && <p className="room-banner" role="status">{launchNotice}</p>}
        {notFound && (
          <div className="controller-dialog-backdrop">
            <div ref={notFoundDialogRef} role="alertdialog" aria-modal="true" aria-labelledby="not-found-title" tabIndex={-1} className="controller-dialog close-room-dialog">
              <header>
                <div>
                  <h2 id="not-found-title">{t("Jogo não encontrado")}</h2>
                  <p>{notFound === "steam"
                    ? t("{game} não está instalado na Steam deste computador. Instale o jogo na Steam e tente de novo.", { game: room.title })
                    : t("O ROM de {game} não está no Fightcade deste computador. Baixe o jogo pelo Fightcade e tente de novo.", { game: room.title })}</p>
                </div>
              </header>
              <footer><span /><button type="button" className="primary-action" onClick={() => setNotFound(null)}>{t("Entendi")}</button></footer>
            </div>
          </div>
        )}
        {choosing && (
          <div className="controller-dialog-backdrop">
            <div ref={chooseDialogRef} role="dialog" aria-modal="true" aria-labelledby="choose-platform-title" tabIndex={-1} className="controller-dialog close-room-dialog">
              <header>
                <div>
                  <h2 id="choose-platform-title">{t("Iniciar {game}", { game: room.title })}</h2>
                  <p>{t("Você tem este jogo em mais de uma plataforma. Por qual quer jogar?")}</p>
                </div>
              </header>
              <div className="controller-dialog-body choose-platform-list">
                {PLAY_ON.filter((p) => choosing.includes(p.id)).map((p) => (
                  <button key={p.id} type="button" className="import-source-option" onClick={() => void launchOn(p.id)}>
                    <span className="platform-mark" aria-hidden="true">{p.label[0]}</span>
                    <span><strong>{t(p.label)}</strong><small>{t(p.id === "steam" ? "Abre pela Steam deste computador" : p.id === "emulator" ? "Abre no Fightcade (FBNeo) deste computador" : "Você inicia o jogo no próprio MiSTer")}</small></span>
                  </button>
                ))}
              </div>
              <footer><span /><button type="button" className="secondary-action" onClick={() => setChoosing(null)}>{t("Cancelar")}</button></footer>
            </div>
          </div>
        )}
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
              <span className="room-player-avatar" aria-hidden="true"><img className="avatar-img" src="/img/avatar-default.png" alt="" /></span>
              <span className="room-player-info">
                <strong>{player.name}{player.you && ` ${t("(você)")}`}</strong>
                <small>{player.score === null ? t("Hi-Score: —") : t("Hi-Score: {score}", { score: player.score.toLocaleString() })}</small>
              </span>
              <span className="room-player-state">
                {player.playing && t("JOGANDO")}
                <span className={`presence-dot is-${player.status}`} role="img" aria-label={t(STATUS_LABEL[player.status])} title={t(STATUS_LABEL[player.status])} />
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
