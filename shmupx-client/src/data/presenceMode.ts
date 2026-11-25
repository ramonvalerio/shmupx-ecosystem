import { useEffect, useState } from "react";

/**
 * Status escolhido no avatar: Online (o automático), Ausente (fica ausente em todas as salas) ou
 * Invisível (aparece offline para os outros e continua jogando, mas sem ver nem usar o chat).
 */
export type PresenceMode = "online" | "away" | "invisible";
const MODES: readonly string[] = ["online", "away", "invisible"];

const KEY = "shmupx.client.presenceMode.v1";
const EVENT = "shmupx:presence-mode";

export function readPresenceMode(): PresenceMode {
  try {
    const saved = localStorage.getItem(KEY);
    return saved && MODES.includes(saved) ? saved as PresenceMode : "online";
  } catch { return "online"; }
}

export function savePresenceMode(mode: PresenceMode) {
  try { localStorage.setItem(KEY, mode); } catch { /* fica só nesta sessão */ }
  window.dispatchEvent(new CustomEvent(EVENT, { detail: mode }));
}

export function usePresenceMode(): PresenceMode {
  const [mode, setMode] = useState<PresenceMode>(readPresenceMode);
  useEffect(() => {
    const onChange = (event: Event) => setMode((event as CustomEvent<PresenceMode>).detail);
    window.addEventListener(EVENT, onChange);
    return () => window.removeEventListener(EVENT, onChange);
  }, []);
  return mode;
}
