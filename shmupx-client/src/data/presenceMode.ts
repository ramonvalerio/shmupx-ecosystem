import { useEffect, useState } from "react";

/** Status escolhido no avatar: Online (o automático) ou Ausente (fica ausente em todas as salas). */
export type PresenceMode = "online" | "away";

const KEY = "shmupx.client.presenceMode.v1";
const EVENT = "shmupx:presence-mode";

export function readPresenceMode(): PresenceMode {
  try { return localStorage.getItem(KEY) === "away" ? "away" : "online"; } catch { return "online"; }
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
