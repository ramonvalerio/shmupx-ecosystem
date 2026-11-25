import type { GameMode, GameRoom } from "../types/room";

/** Modos na ordem em que aparecem para escolher. */
export const GAME_MODES: { id: GameMode; label: string; detail: string }[] = [
  { id: "highscore", label: "Hi-Score", detail: "Pontuação máxima, com quantos créditos quiser." },
  { id: "1cc", label: "1CC", detail: "Zerar com 1 crédito, sem continue." },
  { id: "caravan", label: "Caravan", detail: "Pontuação em tempo limitado." },
];

/**
 * Modos para escolher ao jogar: Hi-Score e Caravan (em breve). O 1CC não se escolhe: a run de
 * Hi-Score que zera com 1 crédito entra também na tabela 1CC do ranking.
 */
export const PLAY_MODES: { id: GameMode; label: string; detail: string; soon?: boolean }[] = [
  { id: "highscore", label: "Hi-Score", detail: "Pontuação máxima. Se zerar com 1 crédito, entra também no ranking 1CC." },
  { id: "caravan", label: "Caravan", detail: "Pontuação em tempo limitado.", soon: true },
];

/**
 * Modos da sala para jogar: sempre os dois. Hi-Score vale para todo jogo (o 1CC sai dele); Caravan
 * só no jogo que o Lab marcou com Caravan (`supported`). Fora dele, o Caravan aparece desabilitado;
 * no jogo com Caravan, desabilitado com "Em breve" até o modo sair.
 */
export function playModesFor(room: GameRoom): (typeof PLAY_MODES[number] & { supported: boolean; disabled: boolean })[] {
  const modes = modesFor(room);
  return PLAY_MODES.map((m) => {
    const supported = m.id === "highscore" || modes.includes(m.id);
    return { ...m, supported, disabled: !supported || Boolean(m.soon) };
  });
}

/** Modo da partida: o escolhido quando a sala tem ele e já está liberado; senão Hi-Score. */
export function playableMode(room: GameRoom, wanted: GameMode | undefined): GameMode {
  return playModesFor(room).some((m) => m.id === wanted && !m.disabled) ? wanted! : "highscore";
}

/** Modos do jogo (os do Lab; sem nenhum marcado, todos): as tabelas do ranking. */
export function modesFor(room: GameRoom): GameMode[] {
  return room.modes && room.modes.length > 0 ? GAME_MODES.map((m) => m.id).filter((id) => room.modes!.includes(id)) : GAME_MODES.map((m) => m.id);
}

/** O modo escolhido no Play Ranked vale para a run que começa quando o jogo abrir. */
const pending = new Map<string, GameMode>();
export function setPendingRunMode(roomId: string, mode: GameMode) { pending.set(roomId, mode); }
/**
 * Modo da partida como o backend recebe (`/api/runs/start`); Caravan = caravana clássica. Um 1CC
 * pedido por um Lite antigo vira Hi-Score (o 1CC sai da run de Hi-Score).
 */
const RUN_MODE: Record<GameMode, string> = { highscore: "highscore", "1cc": "highscore", caravan: "caravan-classic" };
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
