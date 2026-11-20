import { useCallback, useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";

/** Jogo instalado na Steam deste PC (Rust `steam::biblioteca`). */
export interface SteamGame {
  app_id: number;
  name: string;
  installed: boolean;
}

/** Biblioteca importada: os app_ids da Steam deste PC no momento da importação. */
export interface SteamLibrary {
  appIds: number[];
  importedAt: number;
}

const KEY = "shmupx.client.steamLibrary.v1";
const EVENT = "shmupx:steam-library";

export function readSteamLibrary(): SteamLibrary | null {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) ?? "null") as SteamLibrary | null;
    return value && Array.isArray(value.appIds) ? { appIds: value.appIds.filter((id) => Number.isInteger(id) && id > 0), importedAt: Number(value.importedAt) || 0 } : null;
  } catch {
    return null;
  }
}

export function saveSteamLibrary(appIds: number[]) {
  const value: SteamLibrary = { appIds: [...new Set(appIds)], importedAt: Date.now() };
  try { localStorage.setItem(KEY, JSON.stringify(value)); } catch { /* fica só nesta sessão */ }
  window.dispatchEvent(new CustomEvent(EVENT, { detail: value }));
  return value;
}

/** Lê a Steam deste PC (só no app desktop). */
export async function scanSteam(): Promise<{ found: boolean; games: SteamGame[] }> {
  if (!isTauri()) return { found: false, games: [] };
  return invoke<{ found: boolean; games: SteamGame[] }>("steam_biblioteca");
}

/** Biblioteca importada, atualizada em todas as telas quando muda. */
export function useSteamLibrary() {
  const [library, setLibrary] = useState<SteamLibrary | null>(readSteamLibrary);
  useEffect(() => {
    const onChange = (event: Event) => setLibrary((event as CustomEvent<SteamLibrary>).detail);
    window.addEventListener(EVENT, onChange);
    return () => window.removeEventListener(EVENT, onChange);
  }, []);
  /** Lê de novo a Steam e guarda o resultado (botão "Atualizar biblioteca"). */
  const refresh = useCallback(async () => {
    const result = await scanSteam();
    if (result.found) saveSteamLibrary(result.games.filter((g) => g.installed).map((g) => g.app_id));
    return result;
  }, []);
  const owns = useCallback((appId: number) => Boolean(library?.appIds.includes(appId)), [library]);
  return { library, owns, refresh };
}
