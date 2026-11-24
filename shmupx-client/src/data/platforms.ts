import { clearSteamLibrary, saveSteamLibrary, scanSteam, useSteamLibrary } from "./steamLibrary";
import { publish as saveEmulatorLibrary, readEmulatorLibrary, romsFor, scanEmulator, useEmulatorLibrary } from "./emulatorLibrary";
import { useMisterLibrary } from "./misterLibrary";
import type { GameRoom, PlayOn } from "../types/room";

/** Onde se joga no ShmupX, na ordem das etiquetas. */
export const PLAY_ON: { id: PlayOn; label: string }[] = [
  { id: "steam", label: "Steam" },
  { id: "emulator", label: "Emulador" },
  { id: "mister", label: "MiSTer" },
];

/**
 * Plataformas do jogo no catálogo. Jogos salvos antes da escolha múltipla no Lab: pela plataforma
 * antiga e pelo app_id da Steam (mesma regra do Lab).
 */
export function catalogPlayOn(data: { platforms?: unknown; platform?: unknown; steamAppId?: unknown }): PlayOn[] {
  const valid = (value: unknown): value is PlayOn => value === "steam" || value === "emulator" || value === "mister";
  if (Array.isArray(data.platforms) && data.platforms.some(valid)) return PLAY_ON.map((p) => p.id).filter((id) => (data.platforms as unknown[]).includes(id));
  const result: PlayOn[] = [];
  if ((typeof data.steamAppId === "number" && data.steamAppId > 0) || data.platform === "steam") result.push("steam");
  if (data.platform === "arcade" || data.platform === "neogeo" || data.platform === "megadrive") result.push("emulator");
  if (data.platform === "mister") result.push("mister");
  return result;
}

export interface PlatformTag {
  id: PlayOn;
  label: string;
  /** O jogador tem o jogo nessa plataforma (biblioteca importada): a etiqueta fica azul. */
  owned: boolean;
}

/** Bibliotecas importadas do jogador (Steam, emulador, MiSTer), atualizadas em todas as telas. */
export function usePlatformOwnership() {
  const steam = useSteamLibrary();
  const emulator = useEmulatorLibrary();
  const mister = useMisterLibrary();
  const owns = (room: GameRoom, id: PlayOn) =>
    id === "steam" ? room.steamAppId > 0 && steam.owns(room.steamAppId) : id === "emulator" ? emulator.owns(room) : mister.owns(room);
  /** Etiquetas do jogo: as plataformas do catálogo e as em que o jogador já tem o jogo. */
  const tags = (room: GameRoom): PlatformTag[] =>
    PLAY_ON.filter((p) => room.platforms.includes(p.id) || owns(room, p.id)).map((p) => ({ ...p, owned: owns(room, p.id) }));
  const anyLibrary = Boolean(steam.library || emulator.library || mister.library);
  /** Biblioteca importada da plataforma, com atualizar (ler de novo) e desconectar (esquecer). */
  const library = (id: PlayOn) =>
    id === "steam" ? { connected: Boolean(steam.library), refresh: () => steam.refresh(), disconnect: clearSteamLibrary }
    : id === "emulator" ? { connected: Boolean(emulator.library), refresh: () => emulator.refresh(), disconnect: emulator.disconnect }
    : { connected: Boolean(mister.library), refresh: () => mister.refresh(), disconnect: mister.disconnect };
  return { owns, tags, anyLibrary, library };
}

/**
 * Confere, logo antes de iniciar, se o jogo continua instalado neste PC (Steam) ou com o ROM no
 * Fightcade (emulador). Atualiza a biblioteca importada com o que achou: um jogo desinstalado
 * volta ao preto e branco na coleção. O MiSTer é outro aparelho: não é conferido aqui.
 */
export async function stillInstalled(room: GameRoom, platform: PlayOn): Promise<boolean> {
  return (await installCheck(room, platform)) === "ok";
}

/**
 * Como `stillInstalled`, dizendo o que falta: `no-app` = a Steam ou o Fightcade não estão neste PC;
 * `missing` = o jogo (ou o ROM) não está neles.
 */
export async function installCheck(room: GameRoom, platform: PlayOn): Promise<"ok" | "missing" | "no-app"> {
  if (platform === "steam") {
    const result = await scanSteam();
    if (!result.found) return "no-app";
    const installed = result.games.filter((g) => g.installed).map((g) => g.app_id);
    // Só regrava se a Steam já estava importada (iniciar não conecta a biblioteca sozinho).
    if (localStorageHas("shmupx.client.steamLibrary.v1")) saveSteamLibrary(installed, result.games.length);
    return installed.includes(room.steamAppId) ? "ok" : "missing";
  }
  if (platform === "emulator") {
    const current = readEmulatorLibrary();
    const result = await scanEmulator(current?.root || undefined);
    // Sem o emulador (só a pasta de ROMs escolhida), o jogo não abre: é como não ter o Fightcade.
    if (!result.found || result.emulador === false) return "no-app";
    if (current) saveEmulatorLibrary({ root: result.root ?? current.root, roms: result.roms, importedAt: Date.now() });
    return romsFor(room).some((rom) => result.roms.includes(rom)) ? "ok" : "missing";
  }
  return "ok";
}

function localStorageHas(key: string) {
  try { return localStorage.getItem(key) !== null; } catch { return false; }
}
