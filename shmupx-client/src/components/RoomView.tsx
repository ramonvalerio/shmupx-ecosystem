import { useI18n } from "../i18n";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ChatFeed from "./ChatFeed";
import ChatInput from "./ChatInput";
import type { ChatMessage, GameRoom } from "../types/room";

interface RoomViewProps {
  room: GameRoom;
  messages: ChatMessage[];
  isRecording: boolean;
  /** Nome do jogador conectado, listado entre os presentes na sala. */
  playerName: string;
  onSendMessage: (text: string) => void;
  /** Abre o ranking deste jogo na mesma aba da sala. */
  onOpenRanking: () => void;
}

const MOCK_FICHAS_DIGITAIS = 3;

/** Capa do jogo: a arte curada quando existe, senão o cartão tipográfico. */
export function GameArt({ room, className = "" }: { room: GameRoom; className?: string }) {
  const { t } = useI18n();
  return (
    <div className={`game-art art-${room.id} ${className}`}>
      {room.id === "dodonpachi" ? <img src="/img/dodonpachi.png" alt={t("Capa de {game}", { game: room.title })} /> : <span>{room.title}</span>}
    </div>
  );
}

/** Sala dedicada do jogo: cabeçalho com ações, chat no centro e jogadores presentes à direita. */
export default function RoomView({ room, messages, isRecording, playerName, onSendMessage, onOpenRanking }: RoomViewProps) {
  const { t } = useI18n();
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [isLaunching, setIsLaunching] = useState(false);
  const [isStopping, setIsStopping] = useState(false);
  const [showDetails, setShowDetails] = useState(false);

  useEffect(() => {
    setLaunchError(null);
    setIsLaunching(false);
    setIsStopping(false);
    setShowDetails(false);
  }, [room.id]);

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
    mainLabel = "Analisando gameplay...";
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

  return (
    <section className="game-room" aria-label={t("Sala de {game}", { game: room.title })}>
      <header className="game-room-header">
        <GameArt room={room} className="game-room-cover" />
        <div className="game-room-title">
          <h1>{room.title}</h1>
          <p>{t("Presença e chat simulados neste computador · demonstração")}</p>
        </div>
        <div className="game-room-actions">
          <span className="sample-chip">{MOCK_FICHAS_DIGITAIS} {t(" Fichas Digitais · demo ")}</span>
          <button type="button" className="room-action" onClick={onOpenRanking}>{t("Ranking")}</button>
          <button type="button" className={`room-action ${mainVariant}`} onClick={mainAction} disabled={mainDisabled}>{t(mainLabel)}</button>
        </div>
      </header>

      <div className="game-room-main">
        {isRecording && <p className="room-banner recording" role="status"><span className="status-dot" /> {t("GRAVANDO RUN")}</p>}
        {launchError && <p className="room-banner warning" role="alert">{t(launchError)}</p>}
        {room.steamAppId === 0 && <p className="room-banner">{t("Este jogo não tem app_id da Steam no catálogo; o início pela Steam fica indisponível.")}</p>}
        <div className="game-panel-tabs" role="group" aria-label={t("Conteúdo do painel do jogo")}>
          <button type="button" aria-pressed={!showDetails} onClick={() => setShowDetails(false)}>{t("Sala")}</button>
          <button type="button" aria-pressed={showDetails} onClick={() => setShowDetails(true)}>{t("Detalhes")}</button>
        </div>
        <div className="room-chat-panel" hidden={showDetails}>
          <ChatFeed messages={messages} />
          <ChatInput onSendMessage={onSendMessage} />
        </div>
        {showDetails && <div className="game-information">
          <GameArt room={room} />
          <dl className="detail-list"><dt>{t("Plataforma")}</dt><dd>{t("Steam")}</dd><dt>{t("Gravação")}</dt><dd>{isRecording ? t("Run em andamento") : t("Nenhuma run em andamento neste jogo")}</dd><dt>{t("Catálogo")}</dt><dd>{t("Publicado pelo ShmupX Lab.")}</dd></dl>
          <h2>{t("Como jogar")}</h2><p>{t("Inicie o jogo pelo botão acima. A captura começa quando o início da run é autorizado. Durante a gravação, use o botão de parada para encerrar e enviar o resultado.")}</p>
        </div>}
      </div>

      <aside className="game-room-players" aria-label={t("Jogadores na sala")}>
        <h2>{t("Jogadores na sala ({count})", { count: "1" })}</h2>
        <p className="muted">{t("Presença em tempo real ainda não disponível.")}</p>
        <ul>
          <li className="room-player is-you">
            <span className="room-player-avatar" aria-hidden="true">{playerName.slice(0, 2).toUpperCase()}</span>
            <strong>{playerName} {t("(você)")}</strong>
            <small>{isRecording ? t("JOGANDO") : t("ONLINE")}</small>
          </li>
          <li className="room-player">
            <span className="room-player-avatar bot" aria-hidden="true">RX</span>
            <strong>RobotX</strong>
            <small className="bot">{t("BOT")}</small>
          </li>
        </ul>
      </aside>
    </section>
  );
}
