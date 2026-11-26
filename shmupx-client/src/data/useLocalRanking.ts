import { useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Uma run válida guardada neste PC (Rust `capture::ranking_local`). */
export interface LocalRankingEntry {
  run_id: string;
  jogo_id: string;
  score: number;
  vidas_perdidas: number;
  continues: number;
  zerou: boolean;
  /** Rota percorrida (letras maiúsculas seguidas) e bombas usadas, nos jogos em que o motor lê. */
  rota?: string | null;
  bombas?: number | null;
  nave?: string | null;
  sobrevivencia_ms?: number | null;
  modo: string | null;
  /** Quando a run foi aceita (ms). */
  quando: number;
  /** Ficou no ranking online (era o recorde do jogador quando foi enviada). */
  online: boolean;
}

/**
 * Ranking local do jogo: todas as runs válidas da conta neste PC, do maior placar para o menor. O
 * ranking online guarda só o recorde; o histórico fica aqui (e não entra em nenhuma comparação).
 */
export function useLocalRanking(uid: string | undefined, gameId: string, active: boolean): LocalRankingEntry[] {
  const [entries, setEntries] = useState<LocalRankingEntry[]>([]);
  useEffect(() => {
    if (!active || !isTauri()) { setEntries([]); return; }
    let alive = true;
    const load = () => invoke<LocalRankingEntry[]>("ranking_local", { uid: uid ?? null, jogoId: gameId })
      .then((list) => { if (alive) setEntries(list); })
      .catch(() => { if (alive) setEntries([]); });
    void load();
    const stop = listen<string>("ranking-local://mudou", (event) => { if (event.payload === gameId) void load(); });
    return () => { alive = false; void stop.then((unlisten) => unlisten()); };
  }, [uid, gameId, active]);
  return entries;
}
