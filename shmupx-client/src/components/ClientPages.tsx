import { useI18n } from "../i18n";
import { useEffect, useState, type ReactNode } from "react";
import { isTauri, invoke } from "@tauri-apps/api/core";
import { GlobalRankingPreview } from "./RankingPages";
import type { GameRoom } from "../types/room";
import DashboardWorkspace from "./DashboardWorkspace";

export const SECTIONS = ["home", "collection", "rooms", "events", "ranking", "settings", "help", "submissions", "profile", "subscription", "notification"] as const;
export type Section = typeof SECTIONS[number];
export const isSection = (value: string): value is Section => (SECTIONS as readonly string[]).includes(value);


export function PageHeading({ title, detail }: { title: string; detail: string }) {
  const { t } = useI18n();
  return <header className="page-heading"><h1>{t(title)}</h1><p>{t(detail)}</p></header>;
}

function Panel({ title, children, demo = false }: { title: string; children: ReactNode; demo?: boolean }) {
  const { t } = useI18n();
  return <article className="dashboard-panel"><header><h2>{t(title)}</h2>{demo && <span className="sample-chip">{t("DEMO")}</span>}</header>{children}</article>;
}

interface HomeProps {
  rooms: GameRoom[];
  recordingRoomId: string | null;
  onNavigate: (section: Section) => void;
  onOpenGame: (id: string) => void;
}

export function HomePage({ rooms, recordingRoomId, onNavigate, onOpenGame }: HomeProps) {
  const { t } = useI18n();
  const [period, setPeriod] = useState("6");
  const [customizing, setCustomizing] = useState(false);
  const onlineRooms = [...rooms].sort((a, b) => b.playersOnline - a.playersOnline);
  const recording = rooms.find((room) => room.id === recordingRoomId);

  const content: Record<string, ReactNode> = {
    performance: <Panel title={t("Desempenho por modo")}><label className="inline-field">{t("Período")}<select value={period} onChange={(event) => setPeriod(event.target.value)}><option value="6">{t("Últimos 6 meses")}</option><option value="12">{t("Últimos 12 meses")}</option></select></label><div className="chart-empty"><span aria-hidden="true">▥</span><h3>{t("Seu progresso começa na primeira run")}</h3><p>{t("Ainda não há histórico de desempenho disponível neste cliente para os últimos ")}{period} {t(" meses.")}</p><button className="primary-action" onClick={() => onNavigate("collection")}>{t("Escolher um jogo")}</button></div></Panel>,
    games: <Panel title={t("Seus jogos")}><div className="metric"><strong>{rooms.length}</strong><span>{t("jogos no catálogo de demonstração")}</span></div>{rooms.map((room) => <button className="dashboard-row" key={room.id} onClick={() => onOpenGame(room.id)}><span className="game-initial">{room.title.slice(0, 2).toUpperCase()}</span><strong>{room.title}</strong><span aria-hidden="true">→</span></button>)}</Panel>,
    community: <Panel title={t("Jogos da comunidade")} demo>{onlineRooms.map((room, index) => <button className="dashboard-row" key={room.id} onClick={() => onOpenGame(room.id)}><span className="row-position">{index + 1}</span><strong>{room.title}</strong><span>{room.playersOnline} {t(" online")}</span></button>)}<p className="muted">{t("Atividade ilustrativa das salas.")}</p></Panel>,
    ranking: <Panel title={t("Ranking")}><GlobalRankingPreview onMore={() => onNavigate("ranking")} /></Panel>,
    events: <Panel title={t("Eventos")} demo><h3>{t("VERTICAL MASTER CUP")}</h3><p className="muted">{t("Catálogo de eventos do cliente original. Explore os detalhes e os períodos de participação.")}</p><button className="text-action" onClick={() => onNavigate("events")}>{t("Ver eventos →")}</button></Panel>,
  };

  return <div className="client-page"><div className="page-toolbar"><PageHeading title={t("Seu próximo recorde começa aqui.")} detail={t("Seu progresso, seus jogos, do seu jeito.")} /><button className="secondary-action" onClick={() => setCustomizing(!customizing)}>{customizing ? t("Concluir personalização") : t("Personalizar layout")}</button></div>{recording && <p className="recording-notice" role="status">{t("● Run em andamento · ")}{recording.title} <button className="text-action" onClick={() => onOpenGame(recording.id)}>{t("Abrir controles →")}</button></p>}<DashboardWorkspace customizing={customizing}>{content}</DashboardWorkspace></div>;
}

export { RankingPage } from "./RankingPages";

const EVENTS = [
  { id: 3, title: "VERTICAL MASTER CUP", artwork: "vertical_master_cup.png", start: "2026-06-10T20:00:00-03:00", end: "2026-07-10T23:59:00-03:00" },
  { id: 2, title: "BULLET HELL CHAMPIONSHIP", artwork: "bullet_hell_championship.png", start: "2026-04-15T20:00:00-03:00", end: "2026-05-15T23:59:00-03:00" },
  { id: 1, title: "PROLOGO TEMPORADA ZERO", artwork: "shmupx_prolog_season_0.png", start: "2026-02-21T20:00:00-03:00", end: "2026-03-21T23:59:00-03:00" },
];
function eventStatus(event: typeof EVENTS[number], now: number) { return now >= Date.parse(event.end) ? "Encerrado" : now < Date.parse(event.start) ? "Em breve" : "Aberto"; }
function eventDate(value: string, locale: string) { return new Date(value).toLocaleString(locale, { timeZone: "America/Sao_Paulo", dateStyle: "medium", timeStyle: "short" }) + " (UTC−3)"; }

export function EventsPage() {
  const { t, locale } = useI18n();
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("Todos");
  const [selected, setSelected] = useState<number | null>(null);
  const [now, setNow] = useState(Date.now);
  useEffect(() => { const timer = window.setInterval(() => setNow(Date.now()), 30_000); return () => window.clearInterval(timer); }, []);
  const event = EVENTS.find((item) => item.id === selected);
  const visible = EVENTS.filter((item) => item.title.toLowerCase().includes(query.toLowerCase()) && (filter === "Todos" || eventStatus(item, now) === filter));
  return <div className="client-page"><PageHeading title={t("Eventos")} detail={t("Desafios e temporadas da comunidade ShmupX.")} /><p className="muted"><span className="sample-chip">{t("DEMO")}</span> {t(" Catálogo ilustrativo do cliente original; inscrições indisponíveis neste cliente.")}</p>{event ? <><button className="text-action" onClick={() => setSelected(null)}>{t("← Voltar aos eventos")}</button><article className="event-detail"><img src={`/img/events/${event.artwork}`} alt={event.title} /><div><span className="sample-chip">{t(eventStatus(event, now))}</span><h2>{event.title}</h2><dl className="detail-list"><dt>{t("Início do evento")}</dt><dd>{eventDate(event.start, locale)}</dd><dt>{t("Fim das inscrições")}</dt><dd>{eventDate(event.end, locale)}</dd></dl><p className="muted">{t("Os resultados e a inscrição online ainda não estão disponíveis nesta versão.")}</p></div></article></> : <><div className="page-filters"><label>{t("Buscar evento")}<input type="search" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("Nome do evento…")} /></label><label>{t("Status")}<select value={filter} onChange={(e) => setFilter(e.target.value)}>{["Todos", "Aberto", "Em breve", "Encerrado"].map((item) => <option key={item} value={item}>{t(item)}</option>)}</select></label></div><div className="events-grid">{visible.map((item) => <button className="event-card" key={item.id} onClick={() => setSelected(item.id)}><img src={`/img/events/${item.artwork}`} alt="" /><div><span className="sample-chip">{t(eventStatus(item, now))}</span><h2>{t(item.title)}</h2><p>{eventDate(item.start, locale)}</p><span className="text-action">{t("Ver detalhes →")}</span></div></button>)}</div>{visible.length === 0 && <p className="empty-state">{t("Nenhum evento corresponde aos filtros.")}</p>}</>}</div>;
}

export function HelpPage() {
  const { t } = useI18n();
  const [group, setGroup] = useState("Como funciona");
  const [version, setVersion] = useState("Não consultada");
  const [busy, setBusy] = useState(false);
  async function checkVersion() {
    if (!isTauri()) { setVersion("Prévia no navegador · abra o aplicativo para consultar a versão nativa."); return; }
    setBusy(true);
    try { setVersion(await invoke<string>("app_version")); } catch { setVersion("Não foi possível consultar a versão do aplicativo."); } finally { setBusy(false); }
  }
  return <div className="client-page"><PageHeading title={t("Ajuda")} detail={t("Entenda o fluxo de uma run e consulte informações do aplicativo.")} /><div className="settings-layout"><nav className="settings-menu" aria-label={t("Tópicos de ajuda")}>{["Como funciona", "Diagnóstico"].map((item) => <button key={item} aria-current={group === item ? "page" : undefined} onClick={() => setGroup(item)}>{t(item)}</button>)}</nav><article className="dashboard-panel settings-content">{group === "Como funciona" ? <><h2>{t("Da coleção ao placar")}</h2><ol className="help-steps"><li><h3>{t("Escolha seu jogo")}</h3><p>{t("Abra a Coleção e selecione um título para entrar na sala.")}</p></li><li><h3>{t("Inicie pela Steam")}</h3><p>{t("Use o botão da sala. Quando o início da run é autorizado, o cliente acompanha o tempo da ficha.")}</p></li><li><h3>{t("Encerre a run")}</h3><p>{t("Use o botão de parada na sala ou aguarde o limite da ficha. O cliente valida a partida e envia o resultado.")}</p></li></ol><p className="muted">{t("O catálogo, o saldo de fichas e os rankings exibidos ainda são de demonstração.")}</p></> : <><h2>{t("Diagnóstico")}</h2><dl className="detail-list"><dt>{t("Ambiente")}</dt><dd>{isTauri() ? t("Aplicativo desktop") : t("Prévia no navegador")}</dd><dt>{t("Versão nativa")}</dt><dd aria-live="polite">{t(version)}</dd></dl><button className="secondary-action" disabled={busy} onClick={checkVersion}>{busy ? t("Consultando…") : t("Consultar versão")}</button><p className="muted">{t("A inicialização das runs depende da autorização do serviço de fichas.")}</p></>}</article></div></div>;
}
