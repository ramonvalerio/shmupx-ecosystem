import { useCallback, useEffect, useState } from "react";
import { listGames, type CatalogGame } from "../../lib/backend";

/** Catálogo completo (inclusive rascunhos) lido do backend, só depois do login. */
export function useAdminCatalog(enabled: boolean) {
  const [games, setGames] = useState<CatalogGame[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setGames(await listGames());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (enabled) void reload();
    else setGames([]);
  }, [enabled, reload]);

  /** Aplica na lista o jogo que o backend acabou de confirmar, sem nova ida ao servidor. */
  const upsert = useCallback((saved: CatalogGame) => {
    setGames((current) =>
      [...current.filter((game) => game.id !== saved.id), saved].sort((a, b) => a.id.localeCompare(b.id)),
    );
  }, []);

  return { games, loading, error, reload, upsert };
}
