import { useEffect, useState } from "react";
import { useI18n } from "../i18n";
import type { GameMode, GameRoom } from "../types/room";
import { carouselOrder, eventRunning, openEventPage, useEvents, type ClientEvent } from "../data/events";
import { useWallet } from "../data/useWallet";
import { useDialogFocus } from "./useDialogFocus";

const MODE_LABEL: Record<GameMode, string> = { highscore: "Hi-Score", "1cc": "1CC", caravan: "Caravan" };
/** Tempo de cada banner antes de passar para o próximo. */
const ROTATE_MS = 7000;

function formatDay(iso: string | null, locale: string): string {
  if (!iso) return "—";
  return new Date(iso).toLocaleString(locale, { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" });
}

/** Situação do evento para o jogador: inscrito, acontecendo ou começando em breve. */
function eventBadge(event: ClientEvent, t: (key: string) => string): { label: string; tone: string } {
  if (event.joined) return { label: t("Inscrito"), tone: "is-joined" };
  if (eventRunning(event)) return { label: t("Aberto"), tone: "is-open" };
  return { label: t("Em breve"), tone: "is-soon" };
}

/**
 * Carrossel de eventos na sala (os publicados no Lab que ainda não terminaram): um banner por vez,
 * passando sozinho (para com o mouse em cima). PARTICIPAR abre um resumo com o custo em fichas;
 * pago, o jogador já está no evento e a ficha mostra o Event Mode.
 */
export default function EventsCarousel({ room, uid }: { room: GameRoom; uid: string | undefined }) {
  const { t, locale } = useI18n();
  const { events, error, loading } = useEvents(Boolean(uid));
  const ordered = carouselOrder(events, room);
  const [index, setIndex] = useState(0);
  const [paused, setPaused] = useState(false);
  const [open, setOpen] = useState<string | null>(null);
  const count = ordered.length;
  const current = count > 0 ? ordered[index % count] : null;

  useEffect(() => {
    if (paused || count < 2 || open) return;
    const timer = window.setInterval(() => setIndex((i) => (i + 1) % count), ROTATE_MS);
    return () => window.clearInterval(timer);
  }, [paused, count, open]);
  useEffect(() => { if (index >= count && count > 0) setIndex(0); }, [index, count]);

  const selected = open ? events.find((e) => e.id === open) ?? null : null;

  if (!current) {
    return <section className="events-carousel is-empty" aria-label={t("Eventos")}>
      <p className="events-carousel-empty">{loading ? t("Carregando os eventos…") : error ? t("Não foi possível carregar os eventos agora.") : t("Nenhum evento aberto no momento.")}</p>
    </section>;
  }

  const badge = eventBadge(current, t);
  const thisGame = current.game_ids.includes(room.id);
  return (
    <section className="events-carousel" aria-label={t("Eventos")} aria-roledescription="carousel" onMouseEnter={() => setPaused(true)} onMouseLeave={() => setPaused(false)}>
      <article key={current.id} className={`event-banner ${current.banner_url ? "has-image" : ""}`} style={current.banner_url ? { backgroundImage: `url("${current.banner_url}")` } : undefined}>
        <div className="event-banner-shade" />
        <div className="event-banner-body">
          <span className={`event-banner-badge ${badge.tone}`}>{badge.label}</span>
          <h3 title={current.title}>{current.title}</h3>
          <p className="event-banner-meta">
            {t(MODE_LABEL[current.mode] ?? current.mode)} · {thisGame ? t("vale neste jogo") : current.game_titles.join(", ")}
          </p>
          <p className="event-banner-meta">
            {eventRunning(current) ? t("Até {date}", { date: formatDay(current.ends_at, locale) }) : t("Começa {date}", { date: formatDay(current.starts_at, locale) })}
            {" · "}{current.entry_fee > 0 ? t("{count} fichas", { count: current.entry_fee }) : t("Grátis")}
          </p>
        </div>
        <button type="button" className="event-banner-cta" onClick={() => setOpen(current.id)}>{t(current.joined ? "VER EVENTO" : "PARTICIPAR")}</button>
      </article>
      {count > 1 && <div className="events-carousel-nav">
        <button type="button" aria-label={t("Evento anterior")} onClick={() => setIndex((i) => (i - 1 + count) % count)}>‹</button>
        <span className="events-carousel-dots">{ordered.map((e, i) => <button key={e.id} type="button" aria-label={e.title} aria-current={i === index % count} onClick={() => setIndex(i)} />)}</span>
        <button type="button" aria-label={t("Próximo evento")} onClick={() => setIndex((i) => (i + 1) % count)}>›</button>
      </div>}
      {selected && <JoinEventDialog event={selected} room={room} uid={uid} onClose={() => setOpen(null)} />}
    </section>
  );
}

/**
 * PARTICIPAR: o evento em poucas linhas, o link da página com tudo, e quanto custa em fichas.
 * Pagando, o jogador já está inscrito e joga valendo pelo Event Mode da ficha.
 */
function JoinEventDialog({ event, room, uid, onClose }: { event: ClientEvent; room: GameRoom; uid: string | undefined; onClose: () => void }) {
  const { t, locale } = useI18n();
  const { join } = useEvents(Boolean(uid));
  const balance = useWallet(uid);
  const ref = useDialogFocus(true, onClose);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [justJoined, setJustJoined] = useState(false);
  const fee = event.entry_fee;
  const short = balance !== null && fee > 0 && balance < fee;
  const summary = event.description.length > 220 ? `${event.description.slice(0, 217).trimEnd()}…` : event.description;
  const thisGame = event.game_ids.includes(room.id);

  async function pay() {
    setBusy(true);
    setError(null);
    try {
      await join(event.id);
      setJustJoined(true);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="controller-dialog-backdrop" onMouseDown={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div ref={ref} role="dialog" aria-modal="true" aria-labelledby="join-event-title" tabIndex={-1} className="controller-dialog join-event-dialog">
        {event.banner_url && <img className="join-event-banner" src={event.banner_url} alt="" />}
        <header>
          <div>
            <h2 id="join-event-title">{event.title}</h2>
            <p>{t(MODE_LABEL[event.mode] ?? event.mode)} · {formatDay(event.starts_at, locale)} → {formatDay(event.ends_at, locale)}</p>
          </div>
        </header>
        <div className="controller-dialog-body join-event-body">
          {summary && <p>{summary}</p>}
          <dl className="join-event-facts">
            <dt>{t("Jogos")}</dt><dd>{event.game_titles.join(", ")}</dd>
            {event.prizes.length > 0 && <><dt>{t("Premiação")}</dt><dd>{event.prizes.slice(0, 3).join(" · ")}{event.prizes.length > 3 ? " …" : ""}</dd></>}
            <dt>{t("Inscrição")}</dt><dd className="join-event-fee">{fee > 0 ? t("{count} fichas", { count: fee }) : t("Grátis")}{balance !== null && fee > 0 && <small> · {t("seu saldo: {count} fichas", { count: balance })}</small>}</dd>
          </dl>
          {event.page_url && <button type="button" className="text-action" onClick={() => void openEventPage(event.page_url).catch(() => setError(t("Não foi possível abrir a página do evento.")))}>{t("Ver a página do evento com todas as informações ↗")}</button>}
          {(event.joined || justJoined)
            ? <p className="success-text" role="status">{thisGame
                ? t("Você está participando. Para jogar valendo, clique na ficha e escolha o evento.")
                : t("Você está participando. Abra a sala de um dos jogos do evento e clique na ficha para jogar valendo.")}</p>
            : short && <p className="mode-error" role="status">{t("Você não tem fichas suficientes para este evento.")}</p>}
          {error && <p className="mode-error" role="alert">{t(error)}</p>}
        </div>
        <footer>
          <button type="button" className="secondary-action" onClick={onClose}>{t("Fechar")}</button>
          {!(event.joined || justJoined) && <button type="button" className="primary-action" disabled={busy || short} onClick={() => void pay()}>
            {t(busy ? "Inscrevendo…" : fee > 0 ? "Pagar {count} fichas e participar" : "Participar grátis", { count: fee })}
          </button>}
        </footer>
      </div>
    </div>
  );
}
