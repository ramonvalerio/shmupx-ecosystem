import { useCallback, useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { GameRoom } from "../types/room";
import { romsFor } from "./emulatorRoms";

export { romsFor };

/**
 * Biblioteca de emulador (Fightcade/FBNeo): os ROMs `.zip` que o jogador tem. "Ter o jogo" no
 * emulador = ter o ROM (como no Client antigo); a ligação ROM ↔ jogo fica em `emulatorRoms.ts`.
 */

export interface EmulatorLibrary {
  /** Pasta do Fightcade em que os ROMs foram lidos. */
  root: string;
  roms: string[];
  importedAt: number;
}

const KEY = "shmupx.client.emulatorLibrary.v1";
const EVENT = "shmupx:emulator-library";

export function readEmulatorLibrary(): EmulatorLibrary | null {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) ?? "null") as EmulatorLibrary | null;
    return value && Array.isArray(value.roms) ? { root: String(value.root ?? ""), roms: value.roms.filter((r) => typeof r === "string"), importedAt: Number(value.importedAt) || 0 } : null;
  } catch {
    return null;
  }
}

export function publish(value: EmulatorLibrary | null) {
  try {
    if (value) localStorage.setItem(KEY, JSON.stringify(value));
    else localStorage.removeItem(KEY);
  } catch { /* fica só nesta sessão */ }
  window.dispatchEvent(new CustomEvent(EVENT, { detail: value }));
}

/** Lê o Fightcade deste PC (só no app desktop). */
export async function scanEmulator(folder?: string): Promise<{ found: boolean; root?: string | null; roms: string[] }> {
  if (!isTauri()) return { found: false, roms: [] };
  return invoke("emulador_biblioteca", { pasta: folder ?? null });
}

/** Biblioteca do emulador importada, atualizada em todas as telas quando muda. */
export function useEmulatorLibrary() {
  const [library, setLibrary] = useState<EmulatorLibrary | null>(readEmulatorLibrary);
  useEffect(() => {
    const onChange = (event: Event) => setLibrary((event as CustomEvent<EmulatorLibrary | null>).detail);
    window.addEventListener(EVENT, onChange);
    return () => window.removeEventListener(EVENT, onChange);
  }, []);
  /** Lê de novo o Fightcade (na pasta já conhecida) e guarda o resultado. */
  const refresh = useCallback(async () => {
    const result = await scanEmulator(readEmulatorLibrary()?.root || undefined);
    if (result.found) publish({ root: result.root ?? "", roms: result.roms, importedAt: Date.now() });
    return result;
  }, []);
  const disconnect = useCallback(() => publish(null), []);
  const owns = useCallback((room: GameRoom) => Boolean(library && romsFor(room).some((rom) => library.roms.includes(rom))), [library]);
  return { library, owns, refresh, disconnect };
}
