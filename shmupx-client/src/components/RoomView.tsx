import { useI18n } from "../i18n";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import ChatFeed from "./ChatFeed";
import ChatInput from "./ChatInput";
import type { ChatMessage, GameRoom } from "../types/room";
import type { SessionRun } from "../types/submission";
import type { ChatStatus } from "../data/useRoomChat";

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
  /** Run mais recente deste jogo nesta sessão: o aviso mostra o andamento e o desfecho. */
  lastRun?: SessionRun;
  /** Motivo de a última tentativa de gravação não ter começado (ex.: FFmpeg, ficha). */
  startError?: string;
}

/** Filtros do chat (não confundir com as abas de sala): todos ou só a conversa com o RobotX. */
type ChatFilter = "everyone" | "robotx";
type SortBy = "score" | "name";

const ROBOTX = "RobotX";
const MENCAO_ROBOTX = /@robotx\b/i;

/** Arte padrão do card: jogo sem imagem enviada pelo Lab. */
export const DEFAULT_COVER = "/img/default-cover.svg";

/** Capa do jogo: a imagem do Lab, a arte curada antiga ou a arte padrão do ShmupX. */
export function GameArt({ room, className = "", decorative = false, children }: { room: GameRoom; className?: string; decorative?: boolean; children?: ReactNode }) {
  const { t } = useI18n();
  const [failed, setFailed] = useState(false);
  const src = room.coverUrl && !failed ? room.coverUrl : room.id === "dodonpachi" ? "/img/dodonpachi.png" : DEFAULT_COVER;
  const kind = src === DEFAULT_COVER ? "is-default" : src === room.coverUrl ? "has-cover" : "";
  return (
    <div className={`game-art art-${room.id} ${kind} ${className}`}>
      <img src={src} alt={decorative ? "" : t("Capa de {game}", { game: room.title })} loading="lazy" onError={() => setFailed(true)} />
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
export default function RoomView({ room, messages, chatStatus, chatError, playerId, isRecording, playerName, onSendMessage, onOpenRanking, lastRun, startError }: RoomViewProps) {
  const { t } = useI18n();
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [isLaunching, setIsLaunching] = useState(false);
  const [isStopping, setIsStopping] = useState(false);
  const [filter, setFilter] = useState<ChatFilter>("everyone");
  const [sortBy, setSortBy] = useState<SortBy>("score");

  useEffect(() => {
    setLaunchError(null);
    setIsLaunching(false);
    setIsStopping(false);
  }, [room.id]);

  // "Só RobotX": mensagens do RobotX e as que o mencionam. É um filtro de leitura; o chat
  // continua o mesmo para todos.
  const visible = useMemo(
    () => (filter === "everyone" ? messages : messages.filter((m) => m.author === ROBOTX || MENCAO_ROBOTX.test(m.text))),
    [filter, messages],
  );

  function send(text: string) {
    // Na aba do RobotX, a mensagem vai endereçada a ele.
    return onSendMessage(filter === "robotx" && !MENCAO_ROBOTX.test(text) ? `@${ROBOTX} ${text}` : text);
  }

  async function handleStartGame() {
    setLaunchError(null);
    setIsLaunching(true);
    try {
      await invoke("inicializar_jogo_steam", { steamAppId: room.steamAppId });
    } catch (error) {
      setLaunchError(typeof error === "string" ? error : "Falha ao iniciar o jogo via Steam.");
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

  let mainLabel = "Iniciar Jogo via Steam";
  let mainAction = handleStartGame;
  let mainDisabled = isLaunching || room.steamAppId === 0;
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
    mainLabel = "Abrindo Steam...";
    mainVariant = "";
  }

  // Jogador conectado sempre primeiro; o RobotX vem no fim e não conta como jogador. Hi-Score
  // da sala ainda não existe: fica "—" em vez de um número inventado.
  const players = [{ id: "you", name: playerName, score: null as number | null }];
  const sorted = sortBy === "name" ? [...players].sort((a, b) => a.name.localeCompare(b.name)) : players;

  return (
    <section className="game-room" aria-label={t("Sala de {game}", { game: room.title })}>
      <div className="game-room-column">
        <header className="game-room-header">
          <div className="game-room-identity">
            <GameArt room={room} className="game-room-cover" />
            <div className="game-room-title">
              <h1 title={room.title}>{room.title}</h1>
              <p>{t("Chat da sala em tempo real · presença simulada (demonstração)")}</p>
            </div>
          </div>
          <div className="game-room-actions">
            <button type="button" className="room-action" onClick={onOpenRanking}>{t("Ranking")}</button>
            <button type="button" className={`room-action ${mainVariant}`} onClick={mainAction} disabled={mainDisabled}>{t(mainLabel)}</button>
          </div>
          <p className="game-room-tip">
            {t("Dica: para uma captura estável, jogue em janela ou em tela cheia sem bordas. A tela cheia exclusiva pode não ser capturada. A gravação começa quando a run é autorizada.")}
          </p>
        </header>

        {isRecording && <p className="room-banner recording" role="status"><span className="status-dot" /> {t("GRAVANDO RUN")}</p>}
        {launchError && <p className="room-banner warning" role="alert">{t(launchError)}</p>}
        {startError && <p className="room-banner warning" role="alert">{t("A gravação não começou: {reason}", { reason: startError })}</p>}
        {lastRun && <RunOutcome run={lastRun} onOpenRanking={onOpenRanking} />}
        {room.steamAppId === 0 && <p className="room-banner">{t("Este jogo não tem app_id da Steam no catálogo; o início pela Steam fica indisponível.")}</p>}

        <div className="room-chat-panel">
          <div className="room-chat-filters" role="tablist" aria-label={t("Filtro do chat")}>
            <button type="button" role="tab" aria-selected={filter === "everyone"} onClick={() => setFilter("everyone")}>
              <IconPlayers /> {t("Para todos")}
            </button>
            <button type="button" role="tab" aria-selected={filter === "robotx"} onClick={() => setFilter("robotx")}>
              <IconRobot /> {t("Só RobotX")}
            </button>
          </div>
          <ChatFeed messages={visible} status={chatStatus} error={chatStatus === "error" ? chatError : null} playerId={playerId} />
        </div>
        <ChatInput
          onSendMessage={send}
          disabled={chatStatus === "error"}
          error={chatStatus === "error" ? null : chatError}
          placeholder={filter === "robotx" ? t("Mensagem para o RobotX") : t("Mensagem para a sala")}
        />
      </div>

      <aside className="game-room-players" aria-label={t("Jogadores na sala")}>
        <h2>{t("Jogadores na sala ({count})", { count: String(players.length) })}</h2>
        <p className="muted">{t("Presença em tempo real ainda não disponível.")}</p>
        <label className="room-sort">
          <span>{t("Ordenar por")}</span>
          <select value={sortBy} onChange={(event) => setSortBy(event.target.value as SortBy)}>
            <option value="score">{t("Hi-Score")}</option>
            <option value="name">{t("Nome")}</option>
          </select>
        </label>
        <ul>
          {sorted.map((player) => (
            <li key={player.id} className="room-player is-you">
              <span className="room-player-avatar" aria-hidden="true">{player.name.slice(0, 2).toUpperCase()}</span>
              <span className="room-player-info">
                <strong>{player.name} {t("(você)")}</strong>
                <small>{player.score === null ? t("Hi-Score: —") : t("Hi-Score: {score}", { score: player.score.toLocaleString() })}</small>
              </span>
              <span className="room-player-state">{isRecording ? t("JOGANDO") : t("ONLINE · DEMO")}</span>
            </li>
          ))}
          <li className="room-player is-bot">
            <span className="room-player-avatar bot" aria-hidden="true"><IconRobot size={20} /></span>
            <span className="room-player-info"><strong>RobotX</strong></span>
            <span className="room-player-state bot"><IconRobot size={11} /> {t("BOT")}</span>
          </li>
        </ul>
        <button type="button" className="room-watch" disabled title={t("Nenhum jogador ao vivo nesta sala.")}>{t("Assistir live")}</button>
      </aside>
    </section>
  );
}
