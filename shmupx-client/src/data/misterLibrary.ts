import { useCallback, useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { GameRoom } from "../types/room";
import { romsFor } from "./emulatorLibrary";
import { misterAuth } from "./misterPlayer";

/**
 * Biblioteca do MiSTer FPGA, lida pela rede (FTP): os `.mra` de arcade, os ROMs de MAME e os jogos
 * de Mega Drive/Genesis e Neo Geo em `games/`. "Ter o jogo" no MiSTer = ter o `.mra` com o título
 * do jogo, o ROM dele (mesma tabela do emulador) ou o arquivo de console com o título ou o ROM.
 */
export interface MisterLibrary {
  /** IP (ou nome) do MiSTer na rede. */
  host: string;
  mras: string[];
  roms: string[];
  /** Jogos de console: `<pasta>/<arquivo>` em `games/` (`MegaDrive/Earthion (World).md`, `NeoGeo/sonicwi2.neo`). */
  consoles?: string[];
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

/**
 * Arquivo de console do MiSTer (`<pasta>/<arquivo>`) que corresponde ao jogo do catálogo: pelo nome
 * do arquivo como ROM (Neo Geo: `sonicwi2.neo` → Aero Fighters 2) ou pelo título, sem região e
 * versão (`Earthion (World) (v1.1).md` → Earthion).
 */
export function misterConsoleFor(room: GameRoom, library: Pick<MisterLibrary, "consoles">): string | undefined {
  const names = new Set([slug(room.id), slug(room.title)]);
  const roms = romsFor(room);
  return (library.consoles ?? []).find((path) => {
    const file = path.split("/").pop() ?? "";
    const base = file.replace(/\.[^.]+$/, "");
    return roms.includes(base.toLowerCase()) || mraTitles(base).some((title) => names.has(title));
  });
}

/** `.mra`, ROMs e jogos de console do MiSTer que correspondem a um jogo do catálogo. */
export function misterMatches(room: GameRoom, library: Pick<MisterLibrary, "mras" | "roms" | "consoles">): boolean {
  const names = new Set([slug(room.id), slug(room.title)]);
  if (romsFor(room).some((rom) => library.roms.includes(rom))) return true;
  if (library.mras.some((mra) => mraTitles(mra).some((title) => names.has(title)))) return true;
  return misterConsoleFor(room, library) !== undefined;
}

export function readMisterLibrary(): MisterLibrary | null {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) ?? "null") as MisterLibrary | null;
    if (!value || typeof value.host !== "string" || !Array.isArray(value.mras) || !Array.isArray(value.roms)) return null;
    const consoles = Array.isArray(value.consoles) ? value.consoles.filter((c) => typeof c === "string") : [];
    return { host: value.host, mras: value.mras.filter((m) => typeof m === "string"), roms: value.roms.filter((r) => typeof r === "string"), consoles, importedAt: Number(value.importedAt) || 0 };
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

/** O `.mra` do MiSTer que corresponde ao jogo do catálogo (pelo título), se houver. */
export function misterMraFor(room: GameRoom, library: Pick<MisterLibrary, "mras">): string | undefined {
  const names = new Set([slug(room.id), slug(room.title)]);
  return library.mras.find((mra) => mraTitles(mra).some((title) => names.has(title)));
}

/**
 * Menu ShmupX do MiSTer: os jogos do Lab marcados com MiSTer que o aparelho tem (o `.mra` em
 * `_Arcade` ou o arquivo de Mega Drive/Neo Geo em `games/`) entram na lista dele, com os modos do
 * catálogo, e o menu é refeito. Só acrescenta. Devolve os títulos adicionados.
 */
export async function syncMisterMenu(host: string, password: string | undefined, rooms: GameRoom[], library: Pick<MisterLibrary, "mras" | "consoles">): Promise<string[]> {
  if (!isTauri()) return [];
  const jogos = rooms
    .filter((room) => room.platforms.includes("mister"))
    .flatMap((room): { titulo: string; mra?: string; console?: string; modos: string[] }[] => {
      const mra = misterMraFor(room, library);
      if (mra) return [{ titulo: room.title, mra, modos: room.modes ?? [] }];
      const file = misterConsoleFor(room, library);
      return file ? [{ titulo: room.title, console: file, modos: room.modes ?? [] }] : [];
    });
  if (jogos.length === 0) return [];
  return invoke<string[]>("mister_sincronizar_menu", { host, ...misterAuth(password), jogos });
}

/** Lê o MiSTer na rede (só no app desktop). */
export async function scanMister(host: string, password?: string): Promise<{ host: string; mras: string[]; roms: string[]; consoles: string[] }> {
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
