import { useCallback, useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { GameRoom } from "../types/room";

/**
 * Biblioteca de emulador (Fightcade/FBNeo): os ROMs `.zip` que o jogador tem. "Ter o jogo" no
 * emulador = ter o ROM (como no Client antigo). O ROM é ligado ao jogo do catálogo pelo nome do
 * jogo (id ou título normalizado) com esta tabela, a mesma do Client antigo.
 */
const ROM_GAMES: Record<string, string[]> = {
  ket: ["ketsui", "ketsui-kizuna-jigoku-tachi"],
  esprade: ["esprade", "esp-ra-de"],
  espgal: ["espgaluda"],
  espgal2: ["espgaluda-ii", "espgaluda-2"],
  guwange: ["guwange"],
  bgaregga: ["battle-garegga"],
  airduel: ["air-duel"],
  dariusg: ["darius-gaiden"],
  dariusgx: ["darius-gaiden"],
  ddonpach: ["dodonpachi"],
  ddp3: ["dodonpachi-daioujou", "dodonpachi-dai-ou-jou"],
  ddpdoj: ["dodonpachi-daioujou", "dodonpachi-dai-ou-jou"],
  ddpdojblk: ["dodonpachi-daioujou-black-label", "dodonpachi-dai-ou-jou-black-label"],
  gunbird: ["gunbird"],
  gunbird2: ["gunbird-2"],
  blazstar: ["blazing-star"],
  sonicwi2: ["aero-fighters-2", "sonic-wings-2"],
  aerofgt: ["aero-fighters", "sonic-wings"],
  sonicwi: ["aero-fighters", "sonic-wings"],
  samuraia: ["samurai-aces", "sengoku-ace"],
  truxton: ["truxton", "tatsujin"],
  truxton2: ["truxton-ii", "truxton-2", "tatsujin-oh"],
  zedblade: ["zed-blade"],
  batsugun: ["batsugun"],
  batrider: ["armed-police-batrider", "batrider"],
  grindstm: ["grind-stormer"],
  gradius3: ["gradius-iii", "gradius-3"],
  raiden: ["raiden"],
  raiden2: ["raiden-ii", "raiden-2"],
  raidendx: ["raiden-dx"],
  mazinger: ["mazinger-z"],
  akatana: ["akai-katana"],
  cawing: ["carrier-air-wing"],
  airbustr: ["air-buster"],
};

export interface EmulatorLibrary {
  /** Pasta do Fightcade em que os ROMs foram lidos. */
  root: string;
  roms: string[];
  importedAt: number;
}

const KEY = "shmupx.client.emulatorLibrary.v1";
const EVENT = "shmupx:emulator-library";

const slug = (text: string) => text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

/** ROMs que correspondem a um jogo do catálogo. */
export function romsFor(room: GameRoom): string[] {
  const names = new Set([room.id, slug(room.id), slug(room.title)]);
  return Object.entries(ROM_GAMES).filter(([rom, games]) => names.has(rom) || games.some((g) => names.has(g))).map(([rom]) => rom);
}

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
