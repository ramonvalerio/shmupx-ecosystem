import { useEffect, useState, useSyncExternalStore } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { GameRoom } from "../types/room";

/**
 * Vídeo do jogo para a tela do gabinete arcade: por enquanto o trailer da loja da Steam (jogos com
 * app_id). Jogos sem trailer continuam com a arte.
 */
export function useGameVideo(room: GameRoom, enabled = true): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    setUrl(null);
    if (!enabled || !isTauri() || !(room.steamAppId > 0)) return;
    let alive = true;
    invoke<string | null>("steam_trailer", { appId: room.steamAppId })
      .then((found) => { if (alive) setUrl(found ?? null); })
      .catch(() => undefined);
    return () => { alive = false; };
  }, [room.steamAppId, enabled]);
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

// Volume do vídeo do gabinete (0 a 1), lembrado entre as aberturas.
const VOLUME_KEY = "shmupx.client.cabinetVolume.v1";
const volumeListeners = new Set<() => void>();
const readVolume = () => {
  try { const value = Number(localStorage.getItem(VOLUME_KEY)); return localStorage.getItem(VOLUME_KEY) !== null && value >= 0 && value <= 1 ? value : 0.6; } catch { return 0.6; }
};
export function setCabinetVolume(volume: number) {
  try { localStorage.setItem(VOLUME_KEY, String(Math.min(1, Math.max(0, volume)))); } catch { /* fica só nesta sessão */ }
  volumeListeners.forEach((listener) => listener());
}
export function useCabinetVolume(): number {
  return useSyncExternalStore((listener) => { volumeListeners.add(listener); return () => volumeListeners.delete(listener); }, readVolume);
}
