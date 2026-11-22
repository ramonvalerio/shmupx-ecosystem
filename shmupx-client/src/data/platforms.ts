import { useSteamLibrary } from "./steamLibrary";
import { useEmulatorLibrary } from "./emulatorLibrary";
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
  return { owns, tags, anyLibrary };
}
