import { useCallback, useEffect, useState } from "react";
import type { GameMode, GameRoom, PlayOn } from "../types/room";
import { GAME_MODES, modesFor } from "./gameModes";
import { PLAY_ON } from "./platforms";

/**
 * Modo de jogo e plataforma escolhidos para cada jogo, os mesmos no painel da Coleção e no
 * cabeçalho da sala (escolher num lugar reflete no outro). Lembrados entre as aberturas.
 */
export type RoomPlay = { mode?: GameMode; platform?: PlayOn };

const KEY = "shmupx.client.roomPlay.v1";
const EVENT = "shmupx:room-play";

function readAll(): Record<string, RoomPlay> {
  try { return JSON.parse(localStorage.getItem(KEY) ?? "{}") as Record<string, RoomPlay>; } catch { return {}; }
}
function readRoomPlay(roomId: string): RoomPlay {
  return readAll()[roomId] ?? {};
}
function saveRoomPlay(roomId: string, play: RoomPlay) {
  try { localStorage.setItem(KEY, JSON.stringify({ ...readAll(), [roomId]: play })); } catch { /* fica só nesta sessão */ }
  window.dispatchEvent(new CustomEvent(EVENT, { detail: roomId }));
}

/**
 * Escolha do jogo: os modos do catálogo e as plataformas em que dá para abrir. Uma plataforma só
 * pode ser escolhida quando o jogo está sincronizado nela (biblioteca importada); sem nenhuma,
 * `platform` fica `null`.
 */
export function useRoomPlay(room: GameRoom, platforms: PlayOn[], owns: (platform: PlayOn) => boolean) {
  const [saved, setSaved] = useState<RoomPlay>(() => readRoomPlay(room.id));
  useEffect(() => {
    setSaved(readRoomPlay(room.id));
    const onChange = (event: Event) => { if ((event as CustomEvent<string>).detail === room.id) setSaved(readRoomPlay(room.id)); };
    window.addEventListener(EVENT, onChange);
    return () => window.removeEventListener(EVENT, onChange);
  }, [room.id]);

  const modes = GAME_MODES.filter((m) => modesFor(room).includes(m.id));
  const options = PLAY_ON.filter((p) => platforms.includes(p.id));
  const mode: GameMode = saved.mode && modes.some((m) => m.id === saved.mode) ? saved.mode : modes[0]?.id ?? "highscore";
  const platform: PlayOn | null = saved.platform && platforms.includes(saved.platform) && owns(saved.platform)
    ? saved.platform
    : options.find((p) => owns(p.id))?.id ?? null;
  const choose = useCallback((next: RoomPlay) => saveRoomPlay(room.id, { mode, platform: platform ?? undefined, ...next }), [room.id, mode, platform]);
  return { modes, options, mode, platform, choose };
}
