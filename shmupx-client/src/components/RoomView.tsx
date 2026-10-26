import { useI18n } from "../i18n";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ChatFeed from "./ChatFeed";
import ChatInput from "./ChatInput";
import RoomRankingDrawer from "./RoomRankingDrawer";
import { gerarRankingSalaMock } from "../data/mockRoomRanking";
import type { ChatMessage, GameRoom } from "../types/room";

interface RoomViewProps {
  room: GameRoom | null;
  messages: ChatMessage[];
  isRecording: boolean;
  onSendMessage: (text: string) => void;
}

const MOCK_FICHAS_DIGITAIS = 3;

export default function RoomView({
  room,
  messages,
  isRecording,
  onSendMessage,
}: RoomViewProps) {
  const { t } = useI18n();
  const [launchError, setLaunchError] = useState<string | null>(null);
  const [isLaunching, setIsLaunching] = useState(false);
  const [isStopping, setIsStopping] = useState(false);
  const [isRankingOpen, setIsRankingOpen] = useState(false);
  const [showDetails, setShowDetails] = useState(false);

  useEffect(() => {
    setLaunchError(null);
    setIsLaunching(false);
    setIsStopping(false);
    setIsRankingOpen(false);
    setShowDetails(false);
  }, [room?.id]);

  if (!room) {
    return (
      <section className="flex flex-1 flex-col items-center justify-center gap-2 bg-zinc-900/40 text-center">
        <p className="font-arcade text-sm text-fuchsia-400">{t("SHMUPX")}</p>
        <p className="text-sm text-zinc-500">{t("Selecione uma sala para entrar na ação.")}</p>
      </section>
    );
  }

  const activeRoom = room;

  async function handleStartGame() {
    setLaunchError(null);
    setIsLaunching(true);
    try {
      await invoke("inicializar_jogo_steam", { steamAppId: activeRoom.steamAppId });
    } catch (error) {
      setLaunchError(
        typeof error === "string" ? error : "Falha ao iniciar o jogo via Steam.",
      );
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
      setLaunchError(
        typeof error === "string" ? error : "Falha ao interromper a run manualmente.",
      );
    } finally {
      setIsStopping(false);
    }
  }

  const rankingMock = gerarRankingSalaMock(activeRoom.id);

  let mainButtonLabel = "Iniciar Jogo via Steam";
  let mainButtonAction = handleStartGame;
  let mainButtonDisabled = isLaunching;
  let mainButtonVariant: "start" | "stop" | "busy" = "start";

  if (isStopping) {
    mainButtonLabel = "Analisando gameplay...";
    mainButtonDisabled = true;
    mainButtonVariant = "busy";
  } else if (isRecording) {
    mainButtonLabel = isStopping ? "Parando..." : "[ STOP ]";
    mainButtonAction = handleStopGame;
    mainButtonDisabled = isStopping;
    mainButtonVariant = isStopping ? "busy" : "stop";
  } else if (isLaunching) {
    mainButtonLabel = "Abrindo Steam...";
    mainButtonVariant = "busy";
  }

  const mainButtonClasses: Record<typeof mainButtonVariant, string> = {
    start:
      "border-emerald-400 bg-emerald-400/10 text-emerald-300 shadow-[0_0_20px_-6px_rgba(16,185,129,0.7)] hover:bg-emerald-400/20",
    stop:
      "border-red-500 bg-red-500/10 text-red-300 shadow-[0_0_20px_-6px_rgba(239,68,68,0.7)] hover:bg-red-500/20",
    busy: "cursor-not-allowed border-zinc-700 bg-zinc-800/60 text-zinc-500",
  };

  return (
    <section className="flex flex-1 flex-col overflow-hidden bg-zinc-900/40">
      <header className="flex items-center justify-between border-b border-white/5 px-6 py-4">
        <div>
          <p className="text-[10px] uppercase tracking-[0.25em] text-zinc-500">{t("Jogo selecionado · Steam")}</p>
          <h1 className="text-lg font-bold text-zinc-100">{room.title}</h1>
        </div>

        <div className="flex items-center gap-3">
          <div className="rounded-md border border-fuchsia-400/40 bg-fuchsia-400/10 px-3 py-1.5 text-xs font-semibold text-fuchsia-300">
            {MOCK_FICHAS_DIGITAIS} {t(" Fichas Digitais · demo ")}</div>

          <button
            type="button"
            onClick={() => setIsRankingOpen(true)}
            className="rounded-md border border-cyan-400/50 bg-cyan-400/10 px-4 py-1.5 text-xs font-semibold uppercase tracking-wider text-cyan-300 transition-colors hover:bg-cyan-400/20"
          >
            {t(" [ VER RANKING ONLINE ] ")}</button>

          <button
            type="button"
            onClick={mainButtonAction}
            disabled={mainButtonDisabled}
            className={`rounded-md border px-4 py-1.5 text-xs font-semibold uppercase tracking-wider transition-colors ${mainButtonClasses[mainButtonVariant]}`}
          >
            {t(mainButtonLabel)}
          </button>
        </div>
      </header>

      {isRecording && (
        <div className="flex items-center gap-2 border-b border-red-500/30 bg-red-500/10 px-6 py-2">
          <span className="h-2 w-2 animate-pulse rounded-full bg-red-500" />
          <p className="font-arcade text-[10px] text-red-400">{t("GRAVANDO RUN")}</p>
        </div>
      )}

      {launchError && (
        <div className="border-b border-amber-500/30 bg-amber-500/10 px-6 py-2 text-xs text-amber-300">
          {t(launchError)}
        </div>
      )}

      <div className="game-panel-tabs" role="group" aria-label={t("Conteúdo do painel do jogo")}>
        <button type="button" aria-pressed={!showDetails} onClick={() => setShowDetails(false)}>{t("Sala")}</button>
        <button type="button" aria-pressed={showDetails} onClick={() => setShowDetails(true)}>{t("Detalhes")}</button>
      </div>
      <div className="room-chat-panel" hidden={showDetails}>
        <ChatFeed messages={messages} />
        <ChatInput onSendMessage={onSendMessage} />
      </div>
      {showDetails && <div className="game-information">
        <div className={`game-art art-${room.id}`}>{room.id === "dodonpachi" ? <img src="/img/dodonpachi.png" alt={t("Capa de {game}", {game:room.title})} /> : <span>{room.title}</span>}</div>
        <dl className="detail-list"><dt>{t("Plataforma")}</dt><dd>{t("Steam")}</dd><dt>{t("Atividade da sala")}</dt><dd>{room.playersOnline} {t(" jogadores online · demonstração")}</dd><dt>{t("Gravação")}</dt><dd>{isRecording ? t("Run em andamento") : t("Nenhuma run em andamento neste jogo")}</dd><dt>{t("Catálogo")}</dt><dd>{t("Dados de demonstração; biblioteca local ainda não sincronizada.")}</dd></dl>
        <h2>{t("Como jogar")}</h2><p>{t("Inicie o jogo pelo botão acima. A captura começa quando o início da run é autorizado. Durante a gravação, use o botão de parada para encerrar e enviar o resultado.")}</p><p className="muted">{t("A instalação do jogo e a disponibilidade de um perfil de análise ainda não foram verificadas por esta tela.")}</p>
      </div>}

      <RoomRankingDrawer
        open={isRankingOpen}
        gameTitle={activeRoom.title}
        entries={rankingMock}
        onClose={() => setIsRankingOpen(false)}
      />
    </section>
  );
}
