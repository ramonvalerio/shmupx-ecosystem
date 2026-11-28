import { useI18n } from "./i18n";
import { useEffect, useMemo, useRef, useState } from "react";
import { isTauri, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import ClientShell from "./components/ClientShell";
import { loadRuns, saveRuns } from "./data/estadoLocal";
import RoomView, { type RoomViewProps } from "./components/RoomView";
import { useAuth } from "./auth/useAuth";
import LoginScreen from "./components/LoginScreen";
import AccountBlockedScreen from "./components/AccountBlockedScreen";
import { useMyModeration } from "./data/useModeration";
import { auth as firebaseAuth } from "./auth/firebase";
import { useRoomChat } from "./data/useRoomChat";
import { usePresenceMode } from "./data/presenceMode";
import { useRoomPresence } from "./data/useRoomPresence";
import NetplayCenter from "./components/NetplayCenter";
import { roomJoinedAt } from "./data/roomSession";
import { useCatalog } from "./data/useCatalog";
import { applyRunAnalysis, applyRunHighlight, mergePendingRuns, type PendingRun, type RunAnalysisEvent, type RunHighlightEvent, type SessionRun } from "./types/submission";
import { useDialogFocus } from "./components/useDialogFocus";
import { takePendingRunEvent, takePendingRunMode } from "./data/gameModes";
import { useRemoteLaunchHost } from "./data/useRemoteLaunchHost";
import { useRemoteLaunch } from "./data/remoteLaunch";
import { publishPlaying, setAccountPlaying, setClientMisterGame, useClientMisterGame, usePlayingClaim } from "./data/playing";
import { liveActive, useLive } from "./data/useLive";
import { runLibrarySync } from "./data/librarySync";
import { loadRomList, setPublishedRooms, useRomList } from "./data/emulatorLibrary";
import { RomDownloadDialog } from "./components/RomDownloadDialog";
import { publishPlacement } from "./data/playerCard";

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
  // Plataforma por onde cada jogo foi aberto neste PC (para declarar o jogo aberto na conta).
  const [launchPlatforms, setLaunchPlatforms] = useState<Record<string, "steam" | "emulator">>({});
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
  const moderation = useMyModeration(signedUid);
  // Jogar no PC pelo celular (ShmupX Lite): este Client recebe os pedidos da conta.
  const remote = useRemoteLaunchHost(signedUid, { rooms, recordingRoomId, practiceGames });
  const remoteRef = useRef(remote);
  remoteRef.current = remote;

  // Um jogo por vez na conta: o que está aberto (aqui ou no celular) fica no Firestore.
  const accountRemote = useRemoteLaunch(signedUid);
  useEffect(() => setAccountPlaying(accountRemote?.playing), [accountRemote?.playing]);
  const live = useLive();
  // O listener do jogo aberto é registrado uma vez: a ref dá a ele o estado atual da live.
  const liveRef = useRef(live.state);
  liveRef.current = live.state;
  const clientMister = useClientMisterGame();
  const liveGameId = liveActive(live.state) ? live.state.game_id ?? null : null;
  const practiceRoomId = Object.keys(practiceGames)[0] ?? null;
  const ownGame = liveGameId ? { gameId: liveGameId, kind: "live" as const, platform: launchPlatforms[liveGameId] ?? "emulator" }
    : recordingRoomId ? { gameId: recordingRoomId, kind: "ranked" as const, platform: launchPlatforms[recordingRoomId] ?? "steam" }
    : practiceRoomId ? { gameId: practiceRoomId, kind: "training" as const, platform: launchPlatforms[practiceRoomId] ?? "emulator" }
    : null;
  // Jogo aberto por aqui no MiSTer: marca a conta uma vez; o agente do MiSTer assume (renova a cada
  // minuto e libera na volta ao menu, mesmo com o Client fechado).
  useEffect(() => {
    if (!signedUid || !clientMister) return;
    const now = Date.now();
    publishPlaying(signedUid, { gameId: clientMister.gameId, title: clientMister.title, platform: "mister", kind: clientMister.kind, device: "pc", startedAt: now, beatAt: now })
      .catch((error) => console.warn("jogo aberto: falha ao avisar a conta:", error));
  }, [signedUid, clientMister]);
  // A conta deixou de ter esse jogo do MiSTer (o agente avisou a volta ao menu): o PC esquece também.
  const misterSeen = useRef(false);
  useEffect(() => {
    const playing = accountRemote?.playing;
    if (!clientMister) { misterSeen.current = false; return; }
    if (playing?.platform === "mister" && playing.gameId === clientMister.gameId) misterSeen.current = true;
    else if (misterSeen.current) { misterSeen.current = false; setClientMisterGame(null); }
  }, [accountRemote?.playing, clientMister]);
  // A cada login (inclusive ao abrir o Client já logado), importa sozinho os jogos da Steam, os ROMs
  // deste PC (baixando os da lista que faltam) e o MiSTer já conhecido, que também ganha no menu
  // ShmupX os jogos do Lab que faltam. Sair e entrar de novo lê tudo outra vez.
  const librarySyncedFor = useRef<string | null>(null);
  // ROMs que o `roms.json` pode baixar: os dos jogos publicados, acompanhando o catálogo.
  useEffect(() => setPublishedRooms(rooms), [rooms]);
  // Id do jogo → ROM (`roms.json`): lida ao abrir; mudar a lista redesenha as telas que usam o ROM.
  useRomList();
  useEffect(() => { void loadRomList().catch((error) => console.warn("ROMs: falha ao ler o roms.json:", error)); }, []);
  useEffect(() => {
    if (!signedUid) { librarySyncedFor.current = null; return; }
    if (!isTauri() || rooms.length === 0 || librarySyncedFor.current === signedUid) return;
    librarySyncedFor.current = signedUid;
    void runLibrarySync(rooms);
  }, [signedUid, rooms]);

  // Partidas desta conta neste PC: voltam ao abrir (e depois de atualizar) e são guardadas a cada
  // mudança. `runsOwner`: de quem é a lista carregada (antes de carregar, nada é salvo por cima).
  const [runsOwner, setRunsOwner] = useState<string | null>(null);
  useEffect(() => {
    if (!signedUid) return;
    setRuns(loadRuns(signedUid));
    setRunsOwner(signedUid);
  }, [signedUid]);
  useEffect(() => {
    if (signedUid && runsOwner === signedUid) saveRuns(signedUid, runs);
  }, [signedUid, runsOwner, runs]);

  usePlayingClaim(signedUid, ownGame
    ? { ...ownGame, title: rooms.find((room) => room.id === ownGame.gameId)?.title ?? ownGame.gameId, device: "pc" }
    : null);
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
    // Jogo aberto (Steam ou emulador): começa a run, vigiando o processo dele. `rom`: o aberto no
    // ShmupX Emulator (os validados pela memória não gravam vídeo).
    const startRun = (roomId: string, pid: number, steamAppId: number | null, rom: string | null = null) => {
      invoke<number>("iniciar_corrida_safe", { jogoId: roomId, jogoPid: pid, steamAppId, modo: takePendingRunMode(roomId), evento: takePendingRunEvent(roomId), rom })
        .then((nativePid) => {
          const run: SessionRun = { id: crypto.randomUUID(), nativePid, gameId: roomId, startedAt: Date.now(), status: "recording" };
          setRuns((current) => [run, ...current]);
          setStartErrors(({ [roomId]: _, ...rest }) => rest);
          remoteRef.current.runStarted(roomId);
        })
        .catch((error) => {
          console.error("falha ao iniciar a corrida segura:", error);
          const message = typeof error === "string" ? error : String(error);
          setStartErrors((current) => ({ ...current, [roomId]: message }));
          remoteRef.current.runFailed(roomId, message);
          // Live Ranked sem a run ranked (sem ficha, gravação recusada): a live não continua.
          if (liveRef.current.status !== "idle" && liveRef.current.status !== "failed" && liveRef.current.game_id === roomId) {
            invoke("live_encerrar").catch(() => {});
          }
        });
    };

    const unlistenLaunched = listen<GameLaunchedPayload>("steam://game-launched", (event) => {
      const room = roomsRef.current.find((candidate) => candidate.steamAppId === event.payload.app_id);
      if (room) setLaunchPlatforms((current) => ({ ...current, [room.id]: "steam" }));
      if (room && event.payload.treino) setPracticeGames((current) => ({ ...current, [room.id]: event.payload.pid }));
      else if (room) startRun(room.id, event.payload.pid, event.payload.app_id);
    });
    const unlistenEmulator = listen<EmulatorLaunchedPayload>("emulator://game-launched", (event) => {
      const room = roomsRef.current.find((candidate) => candidate.id === event.payload.game_id);
      if (room) setLaunchPlatforms((current) => ({ ...current, [room.id]: "emulator" }));
      if (room && event.payload.treino) setPracticeGames((current) => ({ ...current, [room.id]: event.payload.pid }));
      else if (room) startRun(room.id, event.payload.pid, null, event.payload.rom);
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
      // Run aceita com posição no ranking: a tela final do motor (janela ainda aberta) a mostra.
      if (event.payload.status === "submitted" && typeof event.payload.position === "number") void publishPlacement(event.payload.position);
    });

    // Live Ranked no top 3: o envio da live ao canal oficial da ShmupX.
    const unlistenHighlight = listen<RunHighlightEvent>("run://destaque", (event) => {
      setRuns((current) => applyRunHighlight(current, event.payload));
    });

    return () => {
      unlistenAnalysis.then((unlisten) => unlisten());
      unlistenHighlight.then((unlisten) => unlisten());
      unlistenLaunched.then((unlisten) => unlisten());
      unlistenEmulator.then((unlisten) => unlisten());
      unlistenExpired.then((unlisten) => unlisten());
      unlistenPractice.then((unlisten) => unlisten());
    };
  }, []);

  const expiredRoom = rooms.find((room) => room.id === expiredRoomId) ?? null;

  if (auth.state.status === "loading") return <main className="client-shell login-screen" aria-busy="true" />;
  if (auth.state.status !== "signed-in") return <LoginScreen auth={auth} />;
  // Banida ou desativada no Users Manager do Lab: só o aviso, no lugar do Client.
  if (moderation.banned || moderation.disabled) return <AccountBlockedScreen banned={moderation.banned} email={firebaseAuth.currentUser?.email} onSignOut={auth.signOut} />;
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
      practiceGames={practiceGames}
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
    <RomDownloadDialog rooms={rooms} />

      {/* Treino em dupla online: convites (clique direito num jogador › Convidar para jogar) e a conexão. */}
      <NetplayCenter me={{ uid: profileUid, name: profileName }} rooms={rooms} />

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
  // Invisível: sem chat (nem ler nem escrever).
  const invisible = usePresenceMode() === "invisible";
  const chat = useRoomChat(props.room.id, author, since, !invisible);
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
