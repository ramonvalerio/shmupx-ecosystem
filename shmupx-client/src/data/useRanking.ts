import { useCallback, useEffect, useState } from "react";
import { AMBIENTE } from "../auth/firebase";
import type { GameRanking, GlobalRanking } from "../types/ranking";

/**
 * Endereço do shmupx-backend para as rotas públicas de ranking. `VITE_SHMUPX_BACKEND_URL` força
 * um endereço; senão, Cloud Run em `prd` e o backend local em `dev`.
 */
export const BACKEND_URL: string =
  import.meta.env.VITE_SHMUPX_BACKEND_URL ??
  (AMBIENTE === "prd" ? "https://shmupx-backend-230979440400.us-central1.run.app" : "http://localhost:3000");

export type RankingMode = "highscore" | "1cc" | "caravan";

export type RankingState<T> =
  | { status: "loading"; data?: T }
  | { status: "ready"; data: T }
  | { status: "error"; error: string; data?: T };

async function getJson<T>(path: string, signal: AbortSignal): Promise<T> {
  const response = await fetch(`${BACKEND_URL}${path}`, { signal, headers: { accept: "application/json" } });
  if (!response.ok) {
    const body = await response.json().catch(() => null) as { error?: string } | null;
    throw new Error(body?.error ?? `HTTP ${response.status}`);
  }
  return response.json() as Promise<T>;
}

function useRemote<T>(path: string | null): RankingState<T> & { reload: () => void } {
  const [state, setState] = useState<RankingState<T>>({ status: "loading" });
  const [attempt, setAttempt] = useState(0);
  const reload = useCallback(() => setAttempt((value) => value + 1), []);

  useEffect(() => {
    if (!path) { setState({ status: "loading" }); return; }
    const controller = new AbortController();
    setState((previous) => ({ status: "loading", data: previous.data }));
    getJson<T>(path, controller.signal)
      .then((data) => setState({ status: "ready", data }))
      .catch((error: unknown) => {
        if (controller.signal.aborted) return;
        console.error(`falha ao carregar ${path}:`, error);
        setState({ status: "error", error: error instanceof Error ? error.message : String(error) });
      });
    return () => controller.abort();
  }, [path, attempt]);

  return { ...state, reload };
}

/** Ranking de um jogo (= ranking da sala). `gameId` vazio: nada a carregar ainda. */
export function useGameRanking(gameId: string, mode?: RankingMode, limit = 100) {
  const params = new URLSearchParams({ limite: String(limit) });
  if (mode) params.set("modo", mode);
  return useRemote<GameRanking>(gameId ? `/api/ranking/jogos/${encodeURIComponent(gameId)}?${params}` : null);
}

/** Ranking Global. */
export function useGlobalRanking(limit = 100) {
  return useRemote<GlobalRanking>(`/api/ranking/global?limite=${limit}`);
}
