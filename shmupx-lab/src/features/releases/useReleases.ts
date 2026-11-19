import { useCallback, useEffect, useRef, useState } from "react";
import { listReleases, recordReleaseChange, type ClientRelease } from "../../lib/backend";
import type { ScriptInfo } from "../../lib/validation";

/** Enquanto a tela de Versões está aberta, o rascunho acompanha as alterações feitas no Lab. */
const ATUALIZAR_A_CADA_MS = 15_000;

/**
 * Scripts aprovados que ainda não saíram em nenhuma versão nem estão no rascunho: viram itens
 * automáticos (`script:<jogo>`). Vale a versão aprovada mais nova de cada jogo.
 */
export function pendingScripts(scripts: ScriptInfo[], releases: ClientRelease[]): string[] {
  const latest = new Map<string, string>();
  for (const s of scripts.filter((s) => s.status === "approved")) {
    const atual = latest.get(s.id);
    if (!atual || compareVersions(s.version, atual) > 0) latest.set(s.id, s.version);
  }
  const known = new Set(
    releases.filter((r) => r.status !== "cancelled").flatMap((r) => [...r.scripts, ...r.auto.filter((i) => i.section === "scripts").map((i) => i.text)]),
  );
  return [...latest].map(([id, version]) => `${id}@${version}`).filter((script) => !known.has(script));
}

/** Versões do ShmupX Client lidas do backend, só depois do login. */
export function useReleases(enabled: boolean, scripts: ScriptInfo[] = []) {
  const [releases, setReleases] = useState<ClientRelease[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const syncing = useRef(false);

  const reload = useCallback(async (quiet = false) => {
    if (!quiet) setLoading(true);
    setError(null);
    try {
      setReleases(await listReleases());
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (!quiet) setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!enabled) {
      setReleases([]);
      return;
    }
    void reload();
    const timer = window.setInterval(() => void reload(true), ATUALIZAR_A_CADA_MS);
    return () => window.clearInterval(timer);
  }, [enabled, reload]);

  // Scripts aprovados na pasta validators entram sozinhos no rascunho.
  useEffect(() => {
    if (!enabled || loading || syncing.current) return;
    const pending = pendingScripts(scripts, releases);
    if (pending.length === 0) return;
    syncing.current = true;
    void (async () => {
      try {
        for (const script of pending) {
          const [id] = script.split("@");
          await recordReleaseChange(`script:${id}`, "scripts", script);
        }
        await reload(true);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        syncing.current = false;
      }
    })();
  }, [enabled, loading, scripts, releases, reload]);

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
