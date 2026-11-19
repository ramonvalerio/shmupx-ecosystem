import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";

/** Item do sininho, como o backend devolve em `GET /api/notifications`. */
export interface NotificationSummary {
  id: string;
  kind: string;
  title: string;
  summary: string;
  created_at: string;
  read: boolean;
}

/** Conteúdo completo da notificação (tela de detalhe). */
export interface NotificationDetail extends NotificationSummary {
  version?: string;
  release_at?: string;
  mandatory: boolean;
  features: string[];
  fixes: string[];
  rules: string[];
  scripts: string[];
}

/** Situação desta versão do Client (`GET /api/client/version`). */
export interface VersionStatus {
  latest: string | null;
  update_available: boolean;
  update_required: boolean;
  next?: string;
  next_release_at?: string;
}

const REFRESH_MS = 5 * 60_000;

/** Notificações do jogador logado: lista, contador de não lidas e situação da versão. */
export function useNotifications() {
  const [items, setItems] = useState<NotificationSummary[]>([]);
  const [unread, setUnread] = useState(0);
  const [version, setVersion] = useState<VersionStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      const list = await invoke<{ unread: number; items: NotificationSummary[] }>("notificacoes_listar");
      setItems(list.items);
      setUnread(list.unread);
      setVersion(await invoke<VersionStatus>("versao_situacao"));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    // O token chega ao Rust logo depois do login: uma segunda tentativa cobre essa corrida.
    void reload();
    const retry = window.setTimeout(() => void reload(), 3000);
    const timer = window.setInterval(() => void reload(), REFRESH_MS);
    const onFocus = () => void reload();
    window.addEventListener("focus", onFocus);
    return () => {
      window.clearTimeout(retry);
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
    };
  }, [reload]);

  const markAllRead = useCallback(async () => {
    await invoke("notificacoes_marcar_todas");
    await reload();
  }, [reload]);

  /**
   * Sininho aberto: relê a lista, devolve as não lidas para o painel mostrar e marca tudo como
   * lido no servidor, para que sumam do sininho (o histórico continua com todas).
   */
  const markSeen = useCallback(async (): Promise<NotificationSummary[]> => {
    const list = await invoke<{ unread: number; items: NotificationSummary[] }>("notificacoes_listar");
    const fresh = list.items.filter((item) => !item.read);
    setItems(list.items);
    setUnread(list.unread);
    setError(null);
    if (fresh.length > 0) {
      await invoke("notificacoes_marcar_todas");
      setItems(list.items.map((item) => ({ ...item, read: true })));
      setUnread(0);
    }
    return fresh;
  }, []);

  return { items, unread, version, error, reload, markAllRead, markSeen };
}

export const getNotification = (id: string) => invoke<NotificationDetail>("notificacoes_obter", { id });

export const setEmailNotices = (on: boolean) => invoke("notificacoes_preferencia_email", { ligada: on });
