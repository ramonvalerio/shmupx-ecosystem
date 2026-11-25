import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { misterAuth, readMisterPlayer } from "./misterPlayer";
import { readMisterLibrary } from "./misterLibrary";
import { clearPlaying, getAccountPlaying, getClientMisterGame, setClientMisterGame } from "./playing";

/**
 * Parar o jogo: o mesmo para o STOP do topo e o PARAR JOGO da sala. Run gravando neste PC (encerra
 * a gravação e a análise tira o placar), jogo no MiSTer (volta ao menu; o agente grava a partida e
 * a conta é liberada mesmo com o MiSTer desligado), treino neste PC (fecha o jogo) ou jogo aberto
 * em outro aparelho (libera a conta daqui). Os dois botões mostram "Parando…" juntos.
 */
export async function stopGame({ uid, recording, practicePid }: {
  uid: string | undefined;
  /** Run sendo gravada neste PC. */
  recording: boolean;
  /** Processo do jogo aberto em treino neste PC. */
  practicePid?: number;
}): Promise<void> {
  if (stopping) return;
  setStopping(true);
  try {
    if (recording) {
      await invoke("finalizar_corrida_manual");
      return;
    }
    const playing = getAccountPlaying();
    if (getClientMisterGame() || playing?.platform === "mister") {
      const player = readMisterPlayer();
      const host = player?.host || readMisterLibrary()?.host;
      if (host) await invoke("mister_parar", { host, ...misterAuth(player?.password) }).catch(() => undefined);
      setClientMisterGame(null);
      if (uid) await clearPlaying(uid);
      return;
    }
    if (practicePid !== undefined) {
      await invoke("treino_parar", { pid: practicePid });
      return;
    }
    if (uid) await clearPlaying(uid);
  } finally {
    setStopping(false);
  }
}

let stopping = false;
const listeners = new Set<() => void>();
function setStopping(value: boolean) {
  stopping = value;
  listeners.forEach((listener) => listener());
}
/** `true` enquanto um dos botões de parar está parando o jogo. */
export function useStoppingGame(): boolean {
  return useSyncExternalStore((listener) => { listeners.add(listener); return () => listeners.delete(listener); }, () => stopping);
}
