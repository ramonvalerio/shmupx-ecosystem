import { useCallback, useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { GameRoom } from "../types/room";
import { romsFor } from "./emulatorLibrary";
import { misterAuth } from "./misterPlayer";

/**
 * Biblioteca do MiSTer FPGA, lida pela rede (FTP): os `.mra` de arcade e os ROMs de MAME. "Ter o
 * jogo" no MiSTer = ter o `.mra` com o título do jogo ou o ROM dele (mesma tabela do emulador).
 */
export interface MisterLibrary {
  /** IP (ou nome) do MiSTer na rede. */
  host: string;
  mras: string[];
  roms: string[];
  importedAt: number;
}

const KEY = "shmupx.client.misterLibrary.v1";
const EVENT = "shmupx:mister-library";

const slug = (text: string) => text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/&/g, " and ").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

/**
 * Títulos de um `.mra` para comparar com o catálogo: o nome sem os parênteses/colchetes
 * ("Darius Gaiden", "Ketsui - Kizuna Jigoku Tachi (2003…)") e só o título principal, antes do
 * subtítulo ("Ketsui", "1941").
 */
function mraTitles(mra: string): string[] {
  const base = mra.replace(/\([^)]*\)|\[[^\]]*\]/g, " ").trim();
  const main = base.split(/\s+-\s+|-\s+/)[0];
  return [slug(base), slug(main)].filter(Boolean);
}

/** `.mra` e ROMs do MiSTer que correspondem a um jogo do catálogo. */
export function misterMatches(room: GameRoom, library: Pick<MisterLibrary, "mras" | "roms">): boolean {
  const names = new Set([slug(room.id), slug(room.title)]);
  if (romsFor(room).some((rom) => library.roms.includes(rom))) return true;
  return library.mras.some((mra) => mraTitles(mra).some((title) => names.has(title)));
}

export function readMisterLibrary(): MisterLibrary | null {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) ?? "null") as MisterLibrary | null;
    if (!value || typeof value.host !== "string" || !Array.isArray(value.mras) || !Array.isArray(value.roms)) return null;
    return { host: value.host, mras: value.mras.filter((m) => typeof m === "string"), roms: value.roms.filter((r) => typeof r === "string"), importedAt: Number(value.importedAt) || 0 };
  } catch {
    return null;
  }
}

export function saveMisterLibrary(value: MisterLibrary | null) {
  try {
    if (value) localStorage.setItem(KEY, JSON.stringify(value));
    else localStorage.removeItem(KEY);
  } catch { /* fica só nesta sessão */ }
  window.dispatchEvent(new CustomEvent(EVENT, { detail: value }));
}

/** Lê o MiSTer na rede (só no app desktop). */
export async function scanMister(host: string, password?: string): Promise<{ host: string; mras: string[]; roms: string[] }> {
  if (!isTauri()) throw new Error("A leitura do MiSTer só funciona no aplicativo desktop.");
  return invoke("mister_biblioteca", { host, ...misterAuth(password) });
}

/** Biblioteca do MiSTer importada, atualizada em todas as telas quando muda. */
export function useMisterLibrary() {
  const [library, setLibrary] = useState<MisterLibrary | null>(readMisterLibrary);
  useEffect(() => {
    const onChange = (event: Event) => setLibrary((event as CustomEvent<MisterLibrary | null>).detail);
    window.addEventListener(EVENT, onChange);
    return () => window.removeEventListener(EVENT, onChange);
  }, []);
  /** Lê de novo o MiSTer (no IP já conhecido) e guarda o resultado. */
  const refresh = useCallback(async () => {
    const current = readMisterLibrary();
    if (!current) return null;
    const result = await scanMister(current.host);
    const next = { ...result, importedAt: Date.now() };
    saveMisterLibrary(next);
    return next;
  }, []);
  const disconnect = useCallback(() => saveMisterLibrary(null), []);
  const owns = useCallback((room: GameRoom) => Boolean(library && misterMatches(room, library)), [library]);
  return { library, owns, refresh, disconnect };
}
