import { useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { GameRoom } from "../types/room";
import { autoImportSteam } from "./steamLibrary";
import { autoImportEmulator } from "./emulatorLibrary";
import { readMisterLibrary, saveMisterLibrary, scanMister, syncMisterMenu } from "./misterLibrary";
import { misterAuth, readMisterPlayer } from "./misterPlayer";

/**
 * Importação automática das bibliotecas a cada login (inclusive ao abrir o Client já logado): os
 * jogos instalados na Steam, os ROMs deste PC (pasta escolhida, `roms` do Client e Fightcade) e o
 * MiSTer já conhecido (que também ganha no menu ShmupX os jogos do Lab que faltam). O estado vai
 * para a barra de baixo do Client.
 */
export interface LibrarySyncState {
  running: boolean;
  /** Quando terminou a última atualização. */
  finishedAt: number | null;
  /** Jogos instalados na Steam; `null` = sem Steam neste PC; `"off"` = o jogador desconectou. */
  steam: number | null | "off";
  /** ROMs deste PC; `null` = sem ROMs; `"off"` = o jogador desconectou o emulador. */
  emulator: number | null | "off";
  /** Jogos do MiSTer (arcade e console); `null` = sem MiSTer conhecido; `"offline"` = não respondeu. */
  mister: number | null | "offline";
}

const EVENT = "shmupx:library-sync";
let state: LibrarySyncState = { running: false, finishedAt: null, steam: null, emulator: null, mister: null };
let running: Promise<void> | null = null;
/** O agente do MiSTer é conferido uma vez por sessão do Client (troca de versão é rara). */
let agentChecked = false;

function publish(next: LibrarySyncState) {
  state = next;
  window.dispatchEvent(new CustomEvent(EVENT, { detail: next }));
}

async function syncMister(rooms: GameRoom[]): Promise<number | null | "offline"> {
  const library = readMisterLibrary();
  const player = readMisterPlayer();
  const host = library?.host || player?.host;
  if (!host) return null;
  if (!agentChecked) {
    agentChecked = true;
    // Agente do ShmupX no MiSTer: atualiza sozinho quando há versão nova publicada (só com o MiSTer
    // no menu, para não cortar uma partida; mantém jogadores e tokens).
    invoke<string | null>("mister_atualizar_agente", { host, ...misterAuth(player?.password) })
      .then((version) => { if (version) console.info(`MiSTer: agente do ShmupX atualizado para ${version}`); })
      .catch((error) => console.warn("MiSTer: falha ao atualizar o agente:", error));
  }
  try {
    const result = await scanMister(host, player?.password);
    saveMisterLibrary({ host: result.host, mras: result.mras, roms: result.roms, consoles: result.consoles, importedAt: Date.now() });
    await syncMisterMenu(result.host, player?.password, rooms, result)
      .catch((error) => console.warn("MiSTer: falha ao atualizar o menu ShmupX:", error));
    return result.mras.length + result.consoles.length;
  } catch (error) {
    // MiSTer desligado ou fora da rede: fica a biblioteca da última leitura.
    console.warn("MiSTer: falha ao atualizar a biblioteca:", error);
    return "offline";
  }
}

/** Lê de novo Steam, ROMs e MiSTer. Uma chamada durante outra em andamento espera a mesma. */
export function runLibrarySync(rooms: GameRoom[]): Promise<void> {
  if (!isTauri()) return Promise.resolve();
  if (running) return running;
  publish({ ...state, running: true });
  running = (async () => {
    const [steam, emulator, mister] = await Promise.all([
      autoImportSteam().catch((error) => { console.warn("Steam: falha na importação automática:", error); return null; }),
      autoImportEmulator().catch((error) => { console.warn("ROMs: falha na importação automática:", error); return null; }),
      syncMister(rooms),
    ]);
    publish({ running: false, finishedAt: Date.now(), steam, emulator, mister });
  })().finally(() => { running = null; });
  return running;
}

/** Estado da importação automática, atualizado em todas as telas. */
export function useLibrarySync(): LibrarySyncState {
  const [current, setCurrent] = useState(state);
  useEffect(() => {
    const onChange = (event: Event) => setCurrent((event as CustomEvent<LibrarySyncState>).detail);
    window.addEventListener(EVENT, onChange);
    setCurrent(state);
    return () => window.removeEventListener(EVENT, onChange);
  }, []);
  return current;
}
