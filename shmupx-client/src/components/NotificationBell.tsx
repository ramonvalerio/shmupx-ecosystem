import { useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n";
import type { NotificationSummary } from "../data/useNotifications";

interface NotificationBellProps {
  items: NotificationSummary[];
  unread: number;
  error: string | null;
  /** Abre o histórico de notificações; com `id`, já com aquela notificação aberta. */
  onOpen: (id: string | null) => void;
  /** Painel aberto: devolve as não lidas e as marca como lidas no servidor. */
  onSeen: () => Promise<NotificationSummary[]>;
}

export function formatDate(iso: string, locale: string): string {
  return new Date(iso).toLocaleString(locale, { dateStyle: "short", timeStyle: "short" });
}

/**
 * Sininho no topo: só as notificações ainda não vistas. Abrir o painel conta como visualizar:
 * elas ficam na tela enquanto ele está aberto e somem do sininho depois (o histórico guarda
 * todas). Clicar numa notificação abre o histórico com ela selecionada.
 */
export default function NotificationBell({ items, unread, error, onOpen, onSeen }: NotificationBellProps) {
  const { t, locale } = useI18n();
  const [open, setOpen] = useState(false);
  // O que o jogador está vendo agora: não some da tela no meio da leitura ao virar "lida".
  const [shown, setShown] = useState<NotificationSummary[]>([]);
  const [seenError, setSeenError] = useState(false);
  const anchorRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onClick = (event: MouseEvent) => {
      if (anchorRef.current && !anchorRef.current.contains(event.target as Node)) setOpen(false);
    };
    window.addEventListener("mousedown", onClick);
    return () => window.removeEventListener("mousedown", onClick);
  }, [open]);

  function toggle() {
    if (open) { setOpen(false); return; }
    setShown(items.filter((item) => !item.read));
    setSeenError(false);
    setOpen(true);
    onSeen()
      .then((fresh) => setShown((current) => {
        const known = new Set(current.map((item) => item.id));
        return [...current, ...fresh.filter((item) => !known.has(item.id))]
          .sort((a, b) => b.created_at.localeCompare(a.created_at));
      }))
      .catch((failure) => {
        console.error("falha ao marcar as notificações como vistas:", failure);
        setSeenError(true);
      });
  }

  function openHistory(id: string | null) {
    setOpen(false);
    onOpen(id);
  }

  const label = unread > 0 ? t("Notificações ({count} não lidas)", { count: unread }) : t("Notificações");

  return (
    <div className="notification-anchor" ref={anchorRef}>
      <button type="button" className="notification-bell" aria-label={label} title={label} aria-expanded={open} aria-controls="notification-panel" onClick={toggle}>
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9" />
          <path d="M10.3 21a1.9 1.9 0 0 0 3.4 0" />
        </svg>
        {unread > 0 && <span className="notification-count">{unread > 9 ? "9+" : unread}</span>}
      </button>
      {open && (
        <div id="notification-panel" className="notification-panel" role="dialog" aria-label={t("Notificações")} onKeyDown={(event) => { if (event.key === "Escape") { event.stopPropagation(); setOpen(false); } }}>
          <div className="notification-panel-header">
            <strong>{t("Notificações")}</strong>
            <button type="button" onClick={() => openHistory(null)}>{t("Ver histórico")}</button>
          </div>
          {error && shown.length === 0 ? (
            <p className="muted notification-empty">{t("Não foi possível carregar as notificações agora.")}</p>
          ) : shown.length === 0 ? (
            <p className="muted notification-empty">{t("Nenhuma notificação nova.")}</p>
          ) : (
            <ul className="notification-list">
              {shown.map((item) => (
                <li key={item.id}>
                  <button type="button" className="unread" onClick={() => openHistory(item.id)}>
                    <span className="notification-dot" aria-label={t("Não lida")} />
                    <span className="notification-text">
                      <strong>{item.title}</strong>
                      <span>{item.summary}</span>
                      <time dateTime={item.created_at}>{formatDate(item.created_at, locale)}</time>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
          {seenError && <p className="muted notification-empty" role="status">{t("Não foi possível marcar as notificações como vistas agora.")}</p>}
        </div>
      )}
    </div>
  );
}
