import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/**
 * Como o Client valida as runs ranked do jogo (Rust `validacao_ranked_dos_jogos`, lido de
 * `shmupx-client/src-tauri/jogos-pela-memoria.json` e `scripts-aceitos.json`).
 */
export interface RankedValidation {
  game: string;
  /** ROMs com mapa de memória no ShmupX Emulator: o placar vem do jogo, sem script de vídeo. */
  roms: string[];
  /** Versão do script de vídeo aceita pelo Client. */
  script: string | null;
}

export const getRankedValidation = () => invoke<RankedValidation[]>("validacao_ranked_dos_jogos");

/** Validação ranked por id de jogo; `error` quando as listas do Client não puderam ser lidas. */
export function useRankedValidation() {
  const [byGame, setByGame] = useState<Map<string, RankedValidation> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const reload = useCallback(() => {
    getRankedValidation()
      .then((list) => {
        setByGame(new Map(list.map((item) => [item.game, item])));
        setError(null);
      })
      .catch((e) => setError(e instanceof Error ? e.message : String(e)));
  }, []);
  useEffect(reload, [reload]);
  return { byGame, error, reload };
}
