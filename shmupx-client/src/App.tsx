import { useI18n } from "./i18n";
import { useEffect, useState } from "react";
import { isTauri, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import ClientShell from "./components/ClientShell";
import RoomView from "./components/RoomView";
import { useAuth } from "./auth/useAuth";
import LoginScreen from "./components/LoginScreen";
import { MOCK_MESSAGES, MOCK_ROOMS } from "./data/mockRooms";
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
  const recordingRoomId = runs.find((run) => run.status === "recording")?.gameId ?? null;
  const expiredDialogRef = useDialogFocus(expiredRoomId !== null, () => setExpiredRoomId(null));

  const selectedRoom = MOCK_ROOMS.find((room) => room.id === selectedRoomId) ?? null;
  const messages = selectedRoomId ? messagesByRoom[selectedRoomId] ?? [] : [];

  // Sessão Firebase (Google). O ID token do Firebase NÃO é repassado a
  // `definir_token_autenticacao`: o shmupx-backend atual valida JWT do Supabase. Sem token na
  // sessão Rust, início de run e voz do RobotX recusam localmente até o backend migrar para
  // Firebase Auth.
  const auth = useAuth();

  useEffect(() => {
    if (!isTauri() || !selectedRoomId) return;
    invoke("definir_sala_ativa", { salaId: selectedRoomId }).catch((error) =>
      console.error("falha ao configurar a sala ativa:", error),
    );
  }, [selectedRoomId]);

  useEffect(() => {
    if (!isTauri()) return;
    const unlistenLaunched = listen<GameLaunchedPayload>("steam://game-launched", (event) => {
      const room = MOCK_ROOMS.find((candidate) => candidate.steamAppId === event.payload.app_id);
      if (!room) return;

      invoke<number>("iniciar_corrida_safe", { jogoId: room.id })
        .then((nativePid) => {
          const run: SessionRun = { id: crypto.randomUUID(), nativePid, gameId: room.id, startedAt: Date.now(), status: "recording" };
          setRuns((current) => [run, ...current]);
        })
        .catch((error) => console.error("falha ao iniciar a corrida segura:", error));
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

  function handleSendMessage(text: string) {
    if (!selectedRoomId) return;

    const newMessage: ChatMessage = {
      id: crypto.randomUUID(),
      author: "Você",
      text,
      timestamp: new Date().toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" }),
    };

    setMessagesByRoom((prev) => ({
      ...prev,
      [selectedRoomId]: [...(prev[selectedRoomId] ?? []), newMessage],
    }));
  }

  const expiredRoom = MOCK_ROOMS.find((room) => room.id === expiredRoomId) ?? null;

  if (auth.state.status === "loading") return <main className="client-shell login-screen" aria-busy="true" />;
  if (auth.state.status === "signed-out") return <LoginScreen busy={auth.state.busy ?? false} error={auth.state.error} onSignIn={auth.signIn} />;

  return (
    <>
    <ClientShell profile={auth.state.profile} onProfile={auth.setProfile} onSignOut={auth.signOut} rooms={MOCK_ROOMS} selectedRoomId={selectedRoomId} recordingRoomId={recordingRoomId} onSelectRoom={setSelectedRoomId} runs={runs}>
      <RoomView
        room={selectedRoom}
        messages={messages}
        isRecording={selectedRoomId !== null && recordingRoomId === selectedRoomId}
        onSendMessage={handleSendMessage}

      />
    </ClientShell>

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
