import { useCallback, useEffect, useState } from "react";
import {
  getEnvironment,
  listVideos,
  onDatasetChanged,
  type ValidationDataset,
  type ValidationEnvironment,
} from "../../lib/validation";

function describe(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Python, bibliotecas, yt-dlp e scripts registrados. Só consulta quando `enabled`. */
export function useValidationEnvironment(enabled: boolean) {
  const [environment, setEnvironment] = useState<ValidationEnvironment | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const check = useCallback(async () => {
    setChecking(true);
    setError(null);
    try {
      setEnvironment(await getEnvironment());
    } catch (e) {
      setError(describe(e));
    } finally {
      setChecking(false);
    }
  }, []);

  useEffect(() => {
    if (enabled && !environment && !checking) void check();
    // Só a primeira abertura dispara; depois, o botão "Verificar de novo".
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled]);

  return { environment, checking, error, check };
}

/** Vídeos e métricas do jogo, atualizados pelos eventos do Rust (download, fila, análise). */
export function useValidationDataset(gameId: string) {
  const [dataset, setDataset] = useState<ValidationDataset | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      setDataset(await listVideos(gameId));
      setError(null);
    } catch (e) {
      setError(describe(e));
    }
  }, [gameId]);

  useEffect(() => {
    setDataset(null);
    void reload();
    const unlisten = onDatasetChanged((changed) => {
      if (changed === gameId) void reload();
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [gameId, reload]);

  return { dataset, error, reload };
}
