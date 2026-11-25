import { useCallback, useEffect, useState } from "react";
import { listUsers, type AdminUser, type UserModeration } from "../../lib/backend";

/** A situação (online, ausente…) muda a cada 30 s no Client; a lista acompanha. */
const ATUALIZAR_A_CADA_MS = 20_000;

/** Jogadores cadastrados, lidos do backend enquanto o Users Manager ou o Coins Manager estão abertos. */
export function useUsers(enabled: boolean) {
  const [users, setUsers] = useState<AdminUser[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [updatedAt, setUpdatedAt] = useState<Date | null>(null);

  const reload = useCallback(async (quiet = false) => {
    if (!quiet) setLoading(true);
    try {
      setUsers(await listUsers());
      setUpdatedAt(new Date());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (!quiet) setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!enabled) return;
    void reload();
    const timer = window.setInterval(() => void reload(true), ATUALIZAR_A_CADA_MS);
    return () => window.clearInterval(timer);
  }, [enabled, reload]);

  /** Aplica na lista a moderação que o backend acabou de confirmar. */
  const setModeration = useCallback((uid: string, moderation: UserModeration) => {
    setUsers((current) => current.map((user) => (user.uid === uid ? { ...user, moderation } : user)));
  }, []);

  return { users, loading, error, updatedAt, reload, setModeration };
}
