import { useI18n } from "./i18n";
import { useEffect, useMemo, useRef, useState } from "react";
import { isTauri, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import ClientShell from "./components/ClientShell";
import RoomView, { type RoomViewProps } from "./components/RoomView";
import { useAuth } from "./auth/useAuth";
import LoginScreen from "./components/LoginScreen";
import { useRoomChat } from "./data/useRoomChat";
import { useRoomPresence } from "./data/useRoomPresence";
import { roomJoinedAt } from "./data/roomSession";
import { useCatalog } from "./data/useCatalog";
import { applyRunAnalysis, mergePendingRuns, type PendingRun, type RunAnalysisEvent, type SessionRun } from "./types/submission";
import { useDialogFocus } from "./components/useDialogFocus";
import { takePendingRunMode } from "./data/gameModes";

interface GameLaunchedPayload {
  app_id: number;
  pid: number;
  name: string;
  /** Modo treino: o jogo abriu sem run (sem gravar nem gastar ficha). */
  treino?: boolean;
}

/** Jogo aberto no emulador (Rust `inicializar_jogo_emulador`). */
interface EmulatorLaunchedPayload {
  game_id: string;
  pid: number;
  rom: string;
  treino?: boolean;
}

interface RunExpiredPayload {
  pid: number;
  jogo_id: string;
}

export default function App() {
  const { t } = useI18n();
  const [selectedRoomId, setSelectedRoomId] = useState<string | null>(null);
  const [expiredRoomId, setExpiredRoomId] = useState<string | null>(null);
  const [runs, setRuns] = useState<SessionRun[]>([]);
  // Por jogo: por que a última tentativa de gravação não começou (mostrado na sala).
  const [startErrors, setStartErrors] = useState<Record<string, string>>({});
  // Jogo aberto no MODO TREINO em cada sala (processo): o PARAR JOGO o fecha.
  const [practiceGames, setPracticeGames] = useState<Record<string, number>>({});
  const recordingRoomId = runs.find((run) => run.status === "recording")?.gameId ?? null;
  const expiredDialogRef = useDialogFocus(expiredRoomId !== null, () => setExpiredRoomId(null));

  // Sessão Firebase (Google). useAuth também mantém o ID token vigente na sessão Rust, que o
  // usa nas chamadas ao shmupx-backend.
  const auth = useAuth();

  // Catálogo publicado pelo Lab (Firestore). Só lê depois do login: as Rules exigem sessão.
  const rooms = useCatalog(auth.state.status === "signed-in");
  // O listener da Steam é registrado uma vez; a ref evita que ele enxergue um catálogo antigo.
  const roomsRef = useRef(rooms);
  roomsRef.current = rooms;

  // Runs "Não enviada" guardadas (vídeo no disco): aparecem no ranking local para analisar de novo.
  const signedUid = auth.state.status === "signed-in" ? auth.state.profile.uid : null;
  useEffect(() => {
    if (!isTauri() || !signedUid) return;
    invoke<PendingRun[]>("runs_pendentes", { uid: signedUid })
      .then((pending) => setRuns((current) => mergePendingRuns(current, pending)))
      .catch((error) => console.error("falha ao ler as runs não enviadas:", error));
  }, [signedUid]);


  useEffect(() => {
    if (!isTauri() || !selectedRoomId) return;
    invoke("definir_sala_ativa", { salaId: selectedRoomId }).catch((error) =>
      console.error("falha ao configurar a sala ativa:", error),
    );
  }, [selectedRoomId]);

  useEffect(() => {
    if (!isTauri()) return;
    // Jogo aberto (Steam ou emulador): começa a run, vigiando o processo dele.
    const startRun = (roomId: string, pid: number, steamAppId: number | null) => {
      invoke<number>("iniciar_corrida_safe", { jogoId: roomId, jogoPid: pid, steamAppId, modo: takePendingRunMode(roomId) })
        .then((nativePid) => {
          const run: SessionRun = { id: crypto.randomUUID(), nativePid, gameId: roomId, startedAt: Date.now(), status: "recording" };
          setRuns((current) => [run, ...current]);
          setStartErrors(({ [roomId]: _, ...rest }) => rest);
        })
        .catch((error) => {
          console.error("falha ao iniciar a corrida segura:", error);
          setStartErrors((current) => ({ ...current, [roomId]: typeof error === "string" ? error : String(error) }));
        });
    };

    const unlistenLaunched = listen<GameLaunchedPayload>("steam://game-launched", (event) => {
      const room = roomsRef.current.find((candidate) => candidate.steamAppId === event.payload.app_id);
      if (room && event.payload.treino) setPracticeGames((current) => ({ ...current, [room.id]: event.payload.pid }));
      else if (room) startRun(room.id, event.payload.pid, event.payload.app_id);
    });
    const unlistenEmulator = listen<EmulatorLaunchedPayload>("emulator://game-launched", (event) => {
      const room = roomsRef.current.find((candidate) => candidate.id === event.payload.game_id);
      if (room && event.payload.treino) setPracticeGames((current) => ({ ...current, [room.id]: event.payload.pid }));
      else if (room) startRun(room.id, event.payload.pid, null);
    });

    const unlistenPractice = listen<{ pid: number }>("treino://encerrado", (event) => {
      setPracticeGames((current) => Object.fromEntries(Object.entries(current).filter(([, pid]) => pid !== event.payload.pid)));
    });

    const unlistenExpired = listen<RunExpiredPayload>("run://expired", (event) => {
      const { jogo_id: jogoId } = event.payload;
      setExpiredRoomId(jogoId);
      const endedAt = Date.now();
      setRuns((current) => current.map((run) => run.nativePid === event.payload.pid && run.gameId === jogoId && run.status === "recording" ? { ...run, endedAt, status: "expired" } : run));
    });

    const unlistenAnalysis = listen<RunAnalysisEvent>("run://analysis", (event) => {
      setRuns((current) => applyRunAnalysis(current, event.payload, Date.now()));
    });

    return () => {
      unlistenAnalysis.then((unlisten) => unlisten());
      unlistenLaunched.then((unlisten) => unlisten());
      unlistenEmulator.then((unlisten) => unlisten());
      unlistenExpired.then((unlisten) => unlisten());
      unlistenPractice.then((unlisten) => unlisten());
    };
  }, []);

  const expiredRoom = rooms.find((room) => room.id === expiredRoomId) ?? null;

  if (auth.state.status === "loading") return <main className="client-shell login-screen" aria-busy="true" />;
  if (auth.state.status === "signed-out") return <LoginScreen busy={auth.state.busy ?? false} error={auth.state.error} onSignIn={auth.signIn} onCancel={auth.cancelSignIn} />;
  const profileName = auth.state.profile.displayName;
  const profileUid = auth.state.profile.uid;

  return (
    <>
    <ClientShell
      profile={auth.state.profile}
      onProfile={auth.setProfile}
      onSignOut={auth.signOut}
      rooms={rooms}
      selectedRoomId={selectedRoomId}
      recordingRoomId={recordingRoomId}
      onSelectRoom={setSelectedRoomId}
      runs={runs}
      renderRoom={(room, actions) => (
        <LiveRoomView
          room={room}
          author={{ uid: profileUid, displayName: profileName }}
          isRecording={recordingRoomId === room.id}
          playerName={profileName}
          onOpenRanking={actions.openRanking}
          onShowCollection={actions.showCollection}
          lastRun={runs.find((run) => run.gameId === room.id)}
          startError={startErrors[room.id]}
          practicePid={practiceGames[room.id]}
        />
      )}
    />

      {expiredRoom && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70">
          <div ref={expiredDialogRef} role="dialog" aria-modal="true" aria-labelledby="expired-title" tabIndex={-1} className="w-80 rounded-lg border border-red-500/50 bg-zinc-950 p-6 text-center shadow-[0_0_40px_-10px_rgba(239,68,68,0.6)]">
            <p id="expired-title" className="font-arcade text-sm text-red-400">{t("TEMPO EXPIRADO")}</p>
            <p className="mt-3 text-sm text-zinc-300">
              {t(" Run encerrada — o tempo da ficha em ")}<span className="text-zinc-100">{expiredRoom.title}</span>{t(" ")}
              {t(" acabou e a gravação foi cortada automaticamente. ")}</p>
            <button
              type="button"
              data-dialog-close
              onClick={() => setExpiredRoomId(null)}
              className="mt-5 w-full rounded-md border border-red-500/60 bg-red-500/10 px-4 py-2 text-xs font-semibold uppercase tracking-wider text-red-300 transition-colors hover:bg-red-500/20"
            >
              {t(" Entendi ")}</button>
          </div>
        </div>
      )}
    </>
  );
}

type LiveRoomViewProps = Omit<RoomViewProps, "messages" | "onSendMessage" | "chatStatus" | "chatError" | "playerId" | "others"> & {
  author: { uid: string; displayName: string };
};

/** Sala com o chat em tempo real (Firestore `rooms/{gameId}/messages`). */
function LiveRoomView({ author, ...props }: LiveRoomViewProps) {
  // O chat mostra só desde a entrada na sala; sair pela aba recomeça (ClientShell.closeRoom).
  const since = useMemo(() => roomJoinedAt(author.uid, props.room.id), [author.uid, props.room.id]);
  const chat = useRoomChat(props.room.id, author, since);
  const others = useRoomPresence(props.room.id, author.uid);
  return (
    <RoomView
      {...props}
      others={others}
      messages={chat.messages}
      chatStatus={chat.status}
      chatError={chat.error}
      playerId={author.uid}
      onSendMessage={chat.send}
    />
  );
}
