import { useCallback, useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Estado da live no YouTube (Rust `live::Estado`), atualizado pelo evento `live://estado`. */
export interface LiveState {
  /** `waiting`: abrindo o jogo e esperando a janela dele (a live captura só a janela). */
  status: "idle" | "waiting" | "preparing" | "connecting" | "live" | "ending" | "failed";
  game_id?: string | null;
  watch_url?: string | null;
  public: boolean;
  ends_at_ms?: number | null;
  error?: string | null;
  ended_by_time_limit: boolean;
  silent: boolean;
}

const IDLE: LiveState = { status: "idle", public: false, ended_by_time_limit: false, silent: false };

export const liveActive = (s: LiveState) => s.status !== "idle" && s.status !== "failed";

export function useLive() {
  const [state, setState] = useState<LiveState>(IDLE);

  useEffect(() => {
    if (!isTauri()) return;
    let stop: (() => void) | undefined;
    let alive = true;
    invoke<LiveState>("live_estado").then((s) => alive && setState(s)).catch(() => {});
    listen<LiveState>("live://estado", (event) => setState(event.payload)).then((un) => {
      if (alive) stop = un;
      else un();
    });
    return () => { alive = false; stop?.(); };
  }, []);

  const start = useCallback((gameId: string, title: string, isPublic: boolean, steamAppId?: number) =>
    invoke("live_iniciar", { jogoId: gameId, titulo: title, publica: isPublic, steamAppId: steamAppId && steamAppId > 0 ? steamAppId : null }), []);
  const stop = useCallback(() => invoke("live_encerrar"), []);
  const open = useCallback((url: string) => invoke("live_abrir", { url }), []);
  return { state, start, stop, open };
}

/** Relógio regressivo "mm:ss" até `endsAt` (ms), atualizado a cada segundo. */
export function useCountdown(endsAt?: number | null) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!endsAt) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [endsAt]);
  if (!endsAt) return null;
  const left = Math.max(0, Math.round((endsAt - now) / 1000));
  return `${String(Math.floor(left / 60)).padStart(2, "0")}:${String(left % 60).padStart(2, "0")}`;
}

export interface YouTubeConnection {
  connected: boolean;
  channel_title?: string | null;
}

/** Conexão do canal do YouTube (Configurações › Integrações). */
export function useYouTube() {
  const [connection, setConnection] = useState<YouTubeConnection | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    if (!isTauri()) { setConnection({ connected: false }); return; }
    invoke<YouTubeConnection>("youtube_conexao").then((c) => { setConnection(c); setError(null); }).catch((e) => { setConnection({ connected: false }); setError(String(e)); });
  }, []);
  useEffect(reload, [reload]);

  const run = (command: string) => async () => {
    setBusy(true);
    setError(null);
    try {
      setConnection(await invoke<YouTubeConnection>(command));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  return { connection, busy, error, connect: run("youtube_conectar"), disconnect: run("youtube_desconectar"), reload };
}
