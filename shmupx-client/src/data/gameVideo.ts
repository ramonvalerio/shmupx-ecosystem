import { useEffect, useState, useSyncExternalStore } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { GameRoom } from "../types/room";

/**
 * Vídeo do jogo para a tela do gabinete arcade: por enquanto o trailer da loja da Steam (jogos com
 * app_id). Jogos sem trailer continuam com a arte.
 */
export function useGameVideo(room: GameRoom): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    setUrl(null);
    if (!isTauri() || !(room.steamAppId > 0)) return;
    let alive = true;
    invoke<string | null>("steam_trailer", { appId: room.steamAppId })
      .then((found) => { if (alive) setUrl(found ?? null); })
      .catch(() => undefined);
    return () => { alive = false; };
  }, [room.steamAppId]);
  return url;
}

// Som do gabinete: ligado por padrão; o botão de alto-falante silencia (vale para todos os jogos).
const KEY = "shmupx.client.cabinetMuted.v1";
const listeners = new Set<() => void>();
const readMuted = () => { try { return localStorage.getItem(KEY) === "1"; } catch { return false; } };
export function setCabinetMuted(muted: boolean) {
  try { localStorage.setItem(KEY, muted ? "1" : "0"); } catch { /* fica só nesta sessão */ }
  listeners.forEach((listener) => listener());
}
export function useCabinetMuted(): boolean {
  return useSyncExternalStore((listener) => { listeners.add(listener); return () => listeners.delete(listener); }, readMuted);
}
