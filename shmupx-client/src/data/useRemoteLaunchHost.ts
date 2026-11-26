import { useCallback, useEffect, useRef } from "react";
import { onSnapshot } from "firebase/firestore";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { GameRoom } from "../types/room";
import { readEmulatorLibrary, romsFor } from "./emulatorLibrary";
import { readEmulatorSettings } from "./emulatorSettings";
import { readEmulatorControls } from "./emulatorControls";
import { caravanSecondsOf, setPendingRunEvent, setPendingRunMode } from "./gameModes";
import { rankedLaunchFields } from "./playerCard";
import { stillInstalled, usePlatformOwnership } from "./platforms";
import {
  COMMAND_TTL_MS, DESKTOP_HEARTBEAT_MS, publishDesktop, publishResult, remoteRef,
  type RemoteCommand, type RemotePlatform, type RemoteState,
} from "./remoteLaunch";
import { blocksLaunch, getAccountPlaying } from "./playing";

const describe = (error: unknown) => (typeof error === "string" ? error : error instanceof Error ? error.message : String(error));

interface HostState {
  rooms: GameRoom[];
  /** Sala com run gravando neste PC. */
  recordingRoomId: string | null;
  /** Jogos abertos em treino (sala → processo). */
  practiceGames: Record<string, number>;
}

/**
 * Client do PC como destino do "Jogar no PC" do ShmupX Lite: avisa que está aberto (e o que tem
 * em cada plataforma) e executa os pedidos de abrir ou parar o jogo da conta.
 */
export function useRemoteLaunchHost(uid: string | null, state: HostState) {
  const playable = usePlatformOwnership();
  const latest = useRef(state);
  latest.current = state;
  const ownsRef = useRef(playable.owns);
  ownsRef.current = playable.owns;
  /** Pedido "play" ranqueado à espera da run começar (para avisar o celular se ela falhar). */
  const awaitingRun = useRef<{ id: string; gameId: string } | null>(null);
  const handled = useRef(new Set<string>());

  // Sinal de vida com as plataformas de cada jogo neste PC.
  useEffect(() => {
    if (!uid || !isTauri()) return;
    const beat = () => {
      const owned: Record<string, RemotePlatform[]> = {};
      for (const room of latest.current.rooms) {
        const list = (["steam", "emulator"] as RemotePlatform[]).filter((id) => ownsRef.current(room, id));
        if (list.length) owned[room.id] = list;
      }
      publishDesktop(uid, owned).catch((error) => console.warn("controle remoto: falha ao avisar o PC aberto:", error));
    };
    beat();
    const timer = window.setInterval(beat, DESKTOP_HEARTBEAT_MS);
    return () => window.clearInterval(timer);
  }, [uid, state.rooms.length]);

  useEffect(() => {
    if (!uid || !isTauri()) return;
    const answer = (id: string, status: "started" | "failed" | "stopped", message?: string) =>
      publishResult(uid, { id, status, message }).catch((error) => console.warn("controle remoto: falha ao responder:", error));

    async function execute(command: RemoteCommand) {
      const { rooms, recordingRoomId, practiceGames } = latest.current;
      if (command.action === "stop") {
        try {
          if (recordingRoomId) await invoke("finalizar_corrida_manual");
          else {
            const pid = practiceGames[command.gameId] ?? Object.values(practiceGames)[0];
            if (pid === undefined) { answer(command.id, "failed", "Nenhum jogo aberto no PC."); return; }
            await invoke("treino_parar", { pid });
          }
          answer(command.id, "stopped", "Jogo encerrado no PC.");
        } catch (error) {
          answer(command.id, "failed", describe(error));
        }
        return;
      }
      const room = rooms.find((candidate) => candidate.id === command.gameId);
      if (!room || !command.platform) { answer(command.id, "failed", "Este jogo não está no catálogo do Client."); return; }
      if (recordingRoomId || Object.keys(practiceGames).length > 0) { answer(command.id, "failed", "Já há um jogo aberto no PC. Pare o jogo antes de abrir outro."); return; }
      // Um jogo valendo ranking por vez na conta (ex.: Ranked no MiSTer): Training não trava.
      const open = getAccountPlaying();
      if (blocksLaunch(open, command.kind === "ranked" ? "ranked" : "training")) { answer(command.id, "failed", `Já tem um jogo aberto na sua conta: ${open.title}. Pare esse jogo antes de abrir outro.`); return; }
      const training = command.kind !== "ranked";
      try {
        if (!(await stillInstalled(room, command.platform))) {
          answer(command.id, "failed", command.platform === "steam"
            ? `${room.title} não está instalado na Steam do PC.`
            : `O ROM de ${room.title} não está na pasta de ROMs do PC (ou o ShmupX Emulator não foi encontrado).`);
          return;
        }
        if (!training && command.mode) setPendingRunMode(room.id, command.mode);
        setPendingRunEvent(room.id, null);
        if (command.platform === "steam") {
          if (room.steamAppId === 0) throw "Este jogo não tem app_id da Steam no catálogo.";
          await invoke("inicializar_jogo_steam", { steamAppId: room.steamAppId, treino: training });
        } else {
          await invoke("inicializar_jogo_emulador", { jogoId: room.id, roms: romsFor(room), pasta: readEmulatorLibrary()?.root || null, treino: training, motorVideo: readEmulatorSettings(), motorControles: readEmulatorControls(),
            ...(await rankedLaunchFields(caravanSecondsOf(room), command.mode === "caravan", !training)) });
        }
        awaitingRun.current = training ? null : { id: command.id, gameId: room.id };
        answer(command.id, "started", training
          ? `Abrindo ${room.title} no PC em treino (sem ficha).`
          : `Abrindo ${room.title} no PC valendo ranking: a gravação começa quando o jogo abrir.`);
      } catch (error) {
        answer(command.id, "failed", describe(error));
      }
    }

    return onSnapshot(remoteRef(uid), (snapshot) => {
      const data = snapshot.data() as RemoteState | undefined;
      const command = data?.command;
      if (!command?.id || handled.current.has(command.id)) return;
      handled.current.add(command.id);
      // Já respondido (Client reaberto) ou velho demais: não executa de novo.
      if (data?.result?.id === command.id || Date.now() - command.createdAt > COMMAND_TTL_MS) return;
      void execute(command);
    }, (error) => console.warn("controle remoto indisponível:", error));
  }, [uid]);

  /** A run de um pedido do celular não começou (sem ficha, live aberta…): avisa o celular. */
  const runFailed = useCallback((gameId: string, message: string) => {
    const waiting = awaitingRun.current;
    if (!uid || !waiting || waiting.gameId !== gameId) return;
    awaitingRun.current = null;
    void publishResult(uid, { id: waiting.id, status: "failed", message }).catch(() => undefined);
  }, [uid]);
  const runStarted = useCallback((gameId: string) => {
    if (awaitingRun.current?.gameId === gameId) awaitingRun.current = null;
  }, []);

  return { runFailed, runStarted };
}
