import { useI18n } from "./i18n";
import { useEffect, useRef, useState } from "react";
import { isTauri, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import ClientShell from "./components/ClientShell";
import RoomView from "./components/RoomView";
import { useAuth } from "./auth/useAuth";
import LoginScreen from "./components/LoginScreen";
import { MOCK_MESSAGES } from "./data/mockRooms";
import { useCatalog } from "./data/useCatalog";
import type { ChatMessage } from "./types/room";
import { applyRunAnalysis, type RunAnalysisEvent, type SessionRun } from "./types/submission";
import { useDialogFocus } from "./components/useDialogFocus";

interface GameLaunchedPayload {
  app_id: number;
  pid: number;
  name: string;
}

interface RunExpiredPayload {
  pid: number;
  jogo_id: string;
}

export default function App() {
  const { t } = useI18n();
  const [selectedRoomId, setSelectedRoomId] = useState<string | null>(null);
  const [messagesByRoom, setMessagesByRoom] = useState<Record<string, ChatMessage[]>>(MOCK_MESSAGES);
  const [expiredRoomId, setExpiredRoomId] = useState<string | null>(null);
  const [runs, setRuns] = useState<SessionRun[]>([]);
  // Por jogo: por que a última tentativa de gravação não começou (mostrado na sala).
  const [startErrors, setStartErrors] = useState<Record<string, string>>({});
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


  useEffect(() => {
    if (!isTauri() || !selectedRoomId) return;
    invoke("definir_sala_ativa", { salaId: selectedRoomId }).catch((error) =>
      console.error("falha ao configurar a sala ativa:", error),
    );
  }, [selectedRoomId]);

  useEffect(() => {
    if (!isTauri()) return;
    const unlistenLaunched = listen<GameLaunchedPayload>("steam://game-launched", (event) => {
      const room = roomsRef.current.find((candidate) => candidate.steamAppId === event.payload.app_id);
      if (!room) return;

      invoke<number>("iniciar_corrida_safe", { jogoId: room.id })
        .then((nativePid) => {
          const run: SessionRun = { id: crypto.randomUUID(), nativePid, gameId: room.id, startedAt: Date.now(), status: "recording" };
          setRuns((current) => [run, ...current]);
          setStartErrors(({ [room.id]: _, ...rest }) => rest);
        })
        .catch((error) => {
          console.error("falha ao iniciar a corrida segura:", error);
          setStartErrors((current) => ({ ...current, [room.id]: typeof error === "string" ? error : String(error) }));
        });
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
      unlistenExpired.then((unlisten) => unlisten());
    };
  }, []);

  function handleSendMessage(roomId: string, text: string) {
    const newMessage: ChatMessage = {
      id: crypto.randomUUID(),
      author: "Você",
      text,
      timestamp: new Date().toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" }),
    };

    setMessagesByRoom((prev) => ({
      ...prev,
      [roomId]: [...(prev[roomId] ?? []), newMessage],
    }));
  }

  const expiredRoom = rooms.find((room) => room.id === expiredRoomId) ?? null;

  if (auth.state.status === "loading") return <main className="client-shell login-screen" aria-busy="true" />;
  if (auth.state.status === "signed-out") return <LoginScreen busy={auth.state.busy ?? false} error={auth.state.error} onSignIn={auth.signIn} />;
  const profileName = auth.state.profile.displayName;

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
        <RoomView
          room={room}
          messages={messagesByRoom[room.id] ?? []}
          isRecording={recordingRoomId === room.id}
          playerName={profileName}
          onSendMessage={(text) => handleSendMessage(room.id, text)}
          onOpenRanking={actions.openRanking}
          lastRun={runs.find((run) => run.gameId === room.id)}
          startError={startErrors[room.id]}
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
