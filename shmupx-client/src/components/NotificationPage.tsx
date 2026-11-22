import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import { getNotification, setEmailNotices, type NotificationDetail, type NotificationSummary } from "../data/useNotifications";
import { formatDate } from "./NotificationBell";

interface NotificationPageProps {
  /** Notificação aberta no histórico; `null` mostra só a lista. */
  id: string | null;
  /** Todas as notificações do jogador, lidas ou não (`GET /api/notifications`). */
  items: NotificationSummary[];
  onSelect: (id: string) => void;
  /** Abrir marca como lida: o sininho relê o contador. */
  onRead: () => void;
}

/**
 * Histórico de notificações: todas, lidas ou não (o sininho mostra só as novas), e o detalhe da
 * selecionada (ex.: o que muda na próxima versão do Client).
 */
export default function NotificationPage({ id, items, onSelect, onRead }: NotificationPageProps) {
  const { t, locale } = useI18n();
  const [detail, setDetail] = useState<NotificationDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [emailChoice, setEmailChoice] = useState<"idle" | "saving" | "off" | "failed">("idle");

  useEffect(() => {
    let alive = true;
    setDetail(null);
    setError(null);
    if (!id) return;
    getNotification(id)
      .then((result) => {
        if (!alive) return;
        setDetail(result);
        onRead();
      })
      .catch((e) => alive && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      alive = false;
    };
  }, [id, onRead]);

  async function stopEmails() {
    setEmailChoice("saving");
    try {
      await setEmailNotices(false);
      setEmailChoice("off");
    } catch {
      setEmailChoice("failed");
    }
  }

  const sections: { title: string; items: string[] }[] = detail
    ? [
        { title: t("Novidades"), items: detail.features },
        { title: t("Correções"), items: detail.fixes },
        { title: t("Mudanças de regra"), items: detail.rules },
        { title: t("Scripts de validação"), items: detail.scripts },
      ].filter((section) => section.items.length > 0)
    : [];

  return (
    <section className="client-page notification-page" aria-busy={Boolean(id) && !detail && !error}>
      <header className="page-heading"><h1>{t("Histórico de notificações")}</h1></header>
      {items.length === 0 ? (
        <p className="muted">{t("Nenhuma notificação por enquanto.")}</p>
      ) : (
        <ul className="notification-list notification-detail" aria-label={t("Histórico de notificações")} style={{ padding: 0 }}>
          {items.map((item) => (
            <li key={item.id}>
              <button type="button" className={item.read ? "" : "unread"} aria-current={item.id === id ? "true" : undefined} onClick={() => onSelect(item.id)}>
                {!item.read && <span className="notification-dot" aria-label={t("Não lida")} />}
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
      {id && error && <p className="mode-error" role="alert">{t("Não foi possível abrir a notificação agora.")}</p>}
      {id && !detail && !error && <p className="muted">{t("Carregando…")}</p>}
      {detail && (
        <article className="notification-detail">
          <header className="page-heading">
            <h1>{detail.title}</h1>
            <p>{detail.summary}</p>
            <p className="notification-meta">
              <time dateTime={detail.created_at}>{t("Publicada em {date}", { date: formatDate(detail.created_at, locale) })}</time>
              {detail.release_at && <span>{t("Liberação: {date}", { date: formatDate(detail.release_at, locale) })}</span>}
              {detail.mandatory && <span className="notification-mandatory">{t("Atualização obrigatória")}</span>}
            </p>
          </header>
          {sections.map((section) => (
            <section key={section.title} className="notification-section">
              <h2>{section.title}</h2>
              <ul>{section.items.map((item) => <li key={item}>{item}</li>)}</ul>
            </section>
          ))}
          {detail.mandatory && <p className="muted">{t("Depois da liberação, novas runs só começam com o Client atualizado.")}</p>}
          {detail.kind === "release" && <footer className="notification-email">
            {emailChoice === "off" ? (
              <span className="muted">{t("Pronto: você não vai mais receber estes avisos por e-mail.")}</span>
            ) : (
              <button type="button" className="secondary-action" disabled={emailChoice === "saving"} onClick={() => void stopEmails()}>
                {t("Não receber estes avisos por e-mail")}
              </button>
            )}
            {emailChoice === "failed" && <span className="mode-error" role="alert">{t("Não foi possível salvar a preferência agora.")}</span>}
          </footer>}
        </article>
      )}
    </section>
  );
}
