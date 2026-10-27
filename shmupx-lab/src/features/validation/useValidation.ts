import { useCallback, useEffect, useRef, useState } from "react";
import {
  getEnvironment,
  listVideos,
  onDatasetChanged,
  type ValidationDataset,
  formatRemaining,
  type ValidationEnvironment,
  type ValidationVideo,
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

/**
 * Tempo restante estimado de cada download/análise, pelo ritmo do progresso observado desde que
 * a etapa começou a ser vista nesta tela. Só aparece depois de algum avanço medido.
 */
export function useRemainingTimes(videos: ValidationVideo[]): Map<string, string> {
  const seen = useRef(new Map<string, { fase: string; t0: number; p0: number }>());
  const now = Date.now();
  const remaining = new Map<string, string>();
  const active = new Set<string>();
  for (const video of videos) {
    const { estado } = video;
    if (estado.fase !== "baixando" && estado.fase !== "analisando") continue;
    active.add(video.id);
    const progress = estado.progresso;
    let start = seen.current.get(video.id);
    // Recomeça a medição quando o percentual sai do zero: a preparação (metadados, calibração do
    // script) não avança o percentual e distorceria o ritmo.
    if (!start || start.fase !== estado.fase || progress < start.p0 || (start.p0 === 0 && progress > 0)) {
      start = { fase: estado.fase, t0: now, p0: progress };
      seen.current.set(video.id, start);
    }
    const advanced = progress - start.p0;
    const elapsed = now - start.t0;
    if (advanced >= 1 && elapsed > 3000) remaining.set(video.id, `faltam ${formatRemaining(((100 - progress) * elapsed) / advanced)}`);
  }
  for (const id of seen.current.keys()) if (!active.has(id)) seen.current.delete(id);
  return remaining;
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
