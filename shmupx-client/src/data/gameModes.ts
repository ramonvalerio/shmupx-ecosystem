import { useSyncExternalStore } from "react";
import type { GameMode, GameRoom } from "../types/room";

/** Modos na ordem em que aparecem para escolher. */
export const GAME_MODES: { id: GameMode; label: string; detail: string }[] = [
  { id: "highscore", label: "Hi-Score", detail: "Pontuação máxima, com quantos créditos quiser." },
  { id: "caravan", label: "Caravan", detail: "Pontuação em tempo limitado." },
  { id: "1cc", label: "1CC", detail: "Zerar com 1 crédito, sem continue." },
];

/**
 * Modos para escolher ao jogar: Hi-Score e Caravan. O 1CC não se escolhe: a run de Hi-Score que
 * zera com 1 crédito entra também na tabela 1CC do ranking. O Caravan é uma run ranked em que o
 * jogo fecha no tempo do Caravan do Lab (`caravanSeconds`); o placar feito até ali vale.
 */
export const PLAY_MODES: { id: GameMode; label: string; detail: string; soon?: boolean }[] = [
  { id: "highscore", label: "Hi-Score", detail: "Pontuação máxima. Se zerar com 1 crédito, entra também no ranking 1CC." },
  { id: "caravan", label: "Caravan", detail: "Pontuação em tempo limitado: o jogo fecha quando o tempo acaba." },
];

/** Tempo do Caravan do jogo (Lab), em segundos; padrão: a caravana clássica, 5 min. */
export function caravanSecondsOf(room: GameRoom): number {
  return room.caravanSeconds ?? 300;
}

/** `300` → `5:00`. */
export function formatCaravanTime(seconds: number): string {
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}

/**
 * Modos da sala para jogar: sempre os dois. Hi-Score vale para todo jogo (o 1CC sai dele); Caravan
 * só no jogo que o Lab marcou com Caravan (`supported`). Fora dele, o Caravan aparece desabilitado.
 */
export function playModesFor(room: GameRoom): (typeof PLAY_MODES[number] & { supported: boolean; disabled: boolean })[] {
  // Só o que o Lab marcou: um jogo sem modos marcados (tabelas de todos os modos) não joga Caravan.
  const marked = room.modes ?? [];
  return PLAY_MODES.map((m) => {
    const supported = m.id === "highscore" || marked.includes(m.id);
    // Caravan com o tempo do jogo no botão ("Caravan · 5:00").
    const label = m.id === "caravan" && supported ? `${m.label} · ${formatCaravanTime(caravanSecondsOf(room))}` : m.label;
    return { ...m, label, supported, disabled: !supported || Boolean(m.soon) };
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
export function setPendingRunMode(roomId: string, mode: GameMode) { pending.set(roomId, mode); setLastRankedMode(roomId, mode); }

/** Último modo do Ranked (ou da Live) aberto em cada jogo neste Client: a lista de jogadores da
 *  sala mostra "RANKED HI-SCORE" ou "RANKED CARAVAN". */
const lastRanked = new Map<string, GameMode>();
let lastRankedVersion = 0;
const lastRankedListeners = new Set<() => void>();
export function setLastRankedMode(roomId: string, mode: GameMode) {
  if (lastRanked.get(roomId) === mode) return;
  lastRanked.set(roomId, mode);
  lastRankedVersion += 1;
  lastRankedListeners.forEach((listener) => listener());
}
export function useLastRankedMode(roomId: string | null): GameMode | null {
  useSyncExternalStore((listener) => { lastRankedListeners.add(listener); return () => lastRankedListeners.delete(listener); }, () => lastRankedVersion);
  return roomId ? lastRanked.get(roomId) ?? null : null;
}
/**
 * Modo da partida como o backend recebe (`/api/runs/start`); Caravan = caravana clássica. Um 1CC
 * pedido por um Lite antigo vira Hi-Score (o 1CC sai da run de Hi-Score).
 */
const RUN_MODE: Record<GameMode, string> = { highscore: "highscore", "1cc": "highscore", caravan: "caravan-classic" };
export const runModeOf = (mode: GameMode): string => RUN_MODE[mode];
export function takePendingRunMode(roomId: string): string | null {
  const mode = pending.get(roomId);
  pending.delete(roomId);
  return mode ? RUN_MODE[mode] : null;
}

/**
 * Evento em que a próxima run da sala vale (Event Mode). Toda partida que começa pela ficha grava
 * ou limpa este valor, para um evento escolhido numa tentativa que não abriu não passar para um
 * Ranked comum depois.
 */
const pendingEvent = new Map<string, string>();
export function setPendingRunEvent(roomId: string, eventId: string | null) {
  if (eventId) pendingEvent.set(roomId, eventId);
  else pendingEvent.delete(roomId);
}
export function takePendingRunEvent(roomId: string): string | null {
  const eventId = pendingEvent.get(roomId) ?? null;
  pendingEvent.delete(roomId);
  return eventId;
}

/** Entrada do menu do MiSTer que corresponde ao modo (pelo nome do modo no menu). */
export function misterEntryMatches(menuMode: string, mode: GameMode): boolean {
  const name = menuMode.toLowerCase().replace(/[^a-z0-9]/g, "");
  if (mode === "highscore") return name.includes("highscore") || name.includes("hiscore") || name === "score";
  return name.includes(mode);
}
