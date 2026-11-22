import type { GameMode, GameRoom } from "../types/room";

/** Modos na ordem em que aparecem para escolher. */
export const GAME_MODES: { id: GameMode; label: string; detail: string }[] = [
  { id: "highscore", label: "Hi-Score", detail: "Pontuação máxima, com quantos créditos quiser." },
  { id: "1cc", label: "1CC", detail: "Zerar com 1 crédito, sem continue." },
  { id: "caravan", label: "Caravan", detail: "Pontuação em tempo limitado." },
];

/** Modos do jogo (os do Lab; sem nenhum marcado, todos). */
export function modesFor(room: GameRoom): GameMode[] {
  return room.modes && room.modes.length > 0 ? GAME_MODES.map((m) => m.id).filter((id) => room.modes!.includes(id)) : GAME_MODES.map((m) => m.id);
}

/** O modo escolhido no Play Ranked vale para a run que começa quando o jogo abrir. */
const pending = new Map<string, GameMode>();
export function setPendingRunMode(roomId: string, mode: GameMode) { pending.set(roomId, mode); }
/** Modo da partida como o backend recebe (`/api/runs/start`); Caravan = caravana clássica. */
const RUN_MODE: Record<GameMode, string> = { highscore: "highscore", "1cc": "1cc", caravan: "caravan-classic" };
export function takePendingRunMode(roomId: string): string | null {
  const mode = pending.get(roomId);
  pending.delete(roomId);
  return mode ? RUN_MODE[mode] : null;
}

/** Entrada do menu do MiSTer que corresponde ao modo (pelo nome do modo no menu). */
export function misterEntryMatches(menuMode: string, mode: GameMode): boolean {
  const name = menuMode.toLowerCase().replace(/[^a-z0-9]/g, "");
  if (mode === "highscore") return name.includes("highscore") || name.includes("hiscore") || name === "score";
  return name.includes(mode);
}
