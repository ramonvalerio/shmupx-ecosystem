import { useCallback, useEffect, useState } from "react";
import { listReports, type ChatReport } from "../../lib/backend";

const ATUALIZAR_A_CADA_MS = 30_000;

/** Denúncias do chat, lidas do backend enquanto o Users Manager está aberto. */
export function useReports(enabled: boolean) {
  const [reports, setReports] = useState<ChatReport[]>([]);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      setReports(await listReports());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    if (!enabled) return;
    void reload();
    const timer = window.setInterval(() => void reload(), ATUALIZAR_A_CADA_MS);
    return () => window.clearInterval(timer);
  }, [enabled, reload]);

  /** Aplica a denúncia que o backend acabou de confirmar. */
  const replace = useCallback((report: ChatReport) => {
    setReports((current) => current.map((r) => (r.id === report.id ? report : r)));
  }, []);

  return { reports, error, reload, replace };
}
