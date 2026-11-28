import type { GameRoom } from "../types/room";

/**
 * ROM de cada jogo do catálogo: vem só do `roms.json` da pasta do Client (Rust `roms_auto`), em que
 * a chave é o id do jogo cadastrado no Lab e o nome do ROM é o do `.zip` do download. O Client não
 * tem nome de ROM no código: jogo fora da lista não tem ROM (não é reconhecido nem aberto no emulador).
 * Jogo de console traz também o sistema do MAME que abre o cartucho (`snes`, `genesis`…).
 */
const KEY = "shmupx.client.romList.v2";

/** ROM do jogo e, se for de console, o sistema do MAME (Rust `RomDoJogo`). */
export interface RomEntry {
  rom: string;
  sistema?: string;
}

/** Id do jogo → ROM. Guardado para valer já ao abrir o Client, antes de ler o `roms.json` de novo. */
let romList: Record<string, RomEntry> = readCached();
let version = 0;
const listeners = new Set<() => void>();

function entryOf(value: unknown): RomEntry | null {
  if (!value || typeof value !== "object") return null;
  const { rom, sistema } = value as { rom?: unknown; sistema?: unknown };
  if (typeof rom !== "string" || !rom) return null;
  return typeof sistema === "string" && sistema ? { rom, sistema } : { rom };
}

function readCached(): Record<string, RomEntry> {
  try {
    const value = JSON.parse(globalThis.localStorage?.getItem(KEY) ?? "null") as unknown;
    if (!value || typeof value !== "object") return {};
    return Object.fromEntries(Object.entries(value).flatMap(([id, entry]) => { const e = entryOf(entry); return e ? [[id, e]] : []; }));
  } catch {
    return {};
  }
}

/** Troca a lista (lida do `roms.json`) e avisa quem depende dela. */
export function setRomList(list: Record<string, RomEntry>) {
  romList = Object.fromEntries(Object.entries(list).flatMap(([id, entry]) => { const e = entryOf(entry); return e ? [[id, e]] : []; }));
  version += 1;
  try { globalThis.localStorage?.setItem(KEY, JSON.stringify(romList)); } catch { /* vale só nesta sessão */ }
  listeners.forEach((listener) => listener());
}

/** Para `useSyncExternalStore`: muda a cada lista nova. */
export function romListVersion() {
  return version;
}

export function subscribeRomList(listener: () => void) {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

/** ROMs do jogo pelo id cadastrado no Lab (vazio = o jogo não está no `roms.json`). */
export function romsFor(room: Pick<GameRoom, "id">): string[] {
  const entry = romList[room.id];
  return entry ? [entry.rom] : [];
}

/** Sistema do MAME que abre o cartucho do jogo de console; `null` = jogo de arcade. */
export function systemFor(room: Pick<GameRoom, "id">): string | null {
  return romList[room.id]?.sistema ?? null;
}
