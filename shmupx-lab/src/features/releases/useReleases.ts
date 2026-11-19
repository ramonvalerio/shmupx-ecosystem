import { useCallback, useEffect, useState } from "react";
import { listReleases, type ClientRelease } from "../../lib/backend";

/** Versões do ShmupX Client lidas do backend, só depois do login. */
export function useReleases(enabled: boolean) {
  const [releases, setReleases] = useState<ClientRelease[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setReleases(await listReleases());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (enabled) void reload();
    else setReleases([]);
  }, [enabled, reload]);

  /** Aplica na lista a versão que o backend acabou de confirmar. */
  const upsert = useCallback((saved: ClientRelease) => {
    setReleases((current) => [...current.filter((r) => r.version !== saved.version), saved].sort((a, b) => compareVersions(b.version, a.version)));
  }, []);

  return { releases, loading, error, reload, upsert };
}

/** `1.10.0` > `1.9.0`. */
export function compareVersions(a: string, b: string): number {
  const [x, y] = [a.split(".").map(Number), b.split(".").map(Number)];
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return (x[i] ?? 0) - (y[i] ?? 0);
  return 0;
}

/** Próxima versão sugerida: o minor seguinte ao maior já criado. */
export function suggestNextVersion(releases: ClientRelease[]): string {
  const latest = releases.map((r) => r.version).sort(compareVersions).at(-1);
  if (!latest) return "0.2.0";
  const [major, minor] = latest.split(".").map(Number);
  return `${major}.${minor + 1}.0`;
}
