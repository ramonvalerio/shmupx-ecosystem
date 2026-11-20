import { useI18n } from "../i18n";
import { useEffect, useMemo, useState } from "react";
import type { GameRoom } from "../types/room";
import type { SessionRun } from "../types/submission";
import { defaultCollection, selectCollection, validateCollection, type CollectionPreferences } from "./collectionState";
import ImportGamesDialog from "./ImportGamesDialog";
import { DEFAULT_COVER, GameArt } from "./RoomView";

const KEY = "shmupx.client.collection.v1";
function readPreferences() { try { return validateCollection(JSON.parse(localStorage.getItem(KEY) ?? "null")); } catch { return defaultCollection(); } }

/** Plataformas do catálogo (as mesmas do Lab/backend). */
const PLATFORM_LABELS: Record<string, string> = {
  steam: "Steam", megadrive: "Mega Drive", neogeo: "Neo Geo", arcade: "Arcade", xbox360: "Xbox 360",
  playstation: "PlayStation", switch: "Switch", other: "Outra",
};
/** Plataforma do jogo: a do catálogo; sem ela, Steam quando há app_id. */
export function platformOf(room: GameRoom): string | undefined {
  return room.platform ?? (room.steamAppId > 0 ? "steam" : undefined);
}

const SORTS: { id: CollectionPreferences["sort"]; label: string }[] = [
  { id: "name", label: "Nome" },
  { id: "recorded", label: "Gameplay gravada" },
  { id: "favorites", label: "Favoritos" },
];

const RUN_STATUS: Record<SessionRun["status"], string> = {
  recording: "Gravando", analyzing: "Em análise", submitted: "Enviada", expired: "Tempo esgotado", failed: "Não enviada",
};

function Segmented<T extends string>({ label, value, options, onChange }: { label: string; value: T; options: { id: T; label: string }[]; onChange: (id: T) => void }) {
  const { t } = useI18n();
  return (
    <div className="collection-segment">
      <span>{t(label)}</span>
      <div role="group" aria-label={t(label)}>
        {options.map((option) => <button type="button" key={option.id} aria-pressed={value === option.id} onClick={() => onChange(option.id)}>{t(option.label)}</button>)}
      </div>
    </div>
  );
}

function formatDuration(ms: number) {
  const minutes = Math.max(1, Math.round(ms / 60000));
  return minutes < 60 ? `${minutes}m` : `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}

/** Gabinete arcade com a capa do jogo na tela. */
function ArcadeCabinet({ room }: { room: GameRoom }) {
  const [failed, setFailed] = useState(false);
  useEffect(() => setFailed(false), [room.coverUrl]);
  const cover = room.coverUrl && !failed ? room.coverUrl : room.id === "dodonpachi" ? "/img/dodonpachi.png" : DEFAULT_COVER;
  return (
    <svg className="arcade-cabinet" viewBox="0 0 320 250" role="img" aria-label={room.title}>
      <defs>
        <clipPath id="cabinet-screen"><rect x="38" y="70" width="244" height="104" rx="2" /></clipPath>
        <linearGradient id="cabinet-body" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stopColor="#152126" /><stop offset="1" stopColor="#0b1013" /></linearGradient>
      </defs>
      <path d="M22 4 H298 V186 L316 222 V246 H4 V222 L22 186 Z" fill="url(#cabinet-body)" stroke="#49c9c9" strokeWidth="2" strokeLinejoin="round" />
      <rect x="30" y="12" width="260" height="40" fill="#0d1a1e" stroke="#49c9c9" strokeWidth="1.5" />
      <text x="160" y="38" textAnchor="middle" className="arcade-cabinet-marquee">{room.title.toUpperCase().slice(0, 26)}</text>
      <rect x="32" y="64" width="256" height="116" fill="#05080a" stroke="#2c4a50" strokeWidth="2" />
      <image href={cover} x="38" y="70" width="244" height="104" preserveAspectRatio="xMidYMid slice" clipPath="url(#cabinet-screen)" onError={() => setFailed(true)} />
      <path d="M22 186 H298 L316 222 H4 Z" fill="#101a1e" stroke="#49c9c9" strokeWidth="2" strokeLinejoin="round" />
      <line x1="82" y1="204" x2="82" y2="192" stroke="#d7e4e8" strokeWidth="3" strokeLinecap="round" />
      <circle cx="82" cy="190" r="6" fill="#fd4851" />
      <circle cx="226" cy="204" r="4.5" fill="#57fcff" /><circle cx="242" cy="204" r="4.5" fill="#57fcff" /><circle cx="258" cy="204" r="4.5" fill="#fd4851" />
      <line x1="140" y1="236" x2="180" y2="236" stroke="#49c9c9" strokeWidth="2" opacity=".6" />
    </svg>
  );
}

export default function CollectionPane({ rooms, runs, selectedRoomId, recordingRoomId, onSelectRoom, onOpenRoom, onOpenRanking, compact, bigPicture }: {
  rooms: GameRoom[]; runs: SessionRun[]; selectedRoomId: string | null; recordingRoomId: string | null;
  onSelectRoom: (id: string) => void;
  /** Abre a sala do jogo numa aba (duplo clique ou Enter no card). */
  onOpenRoom: (id: string) => void;
  /** Abre a sala já na aba do ranking do jogo. */
  onOpenRanking?: (id: string) => void;
  compact: boolean; bigPicture: boolean;
}) {
  const { t, locale } = useI18n();
  const [preferences, setPreferences] = useState(readPreferences);
  const [query, setQuery] = useState("");
  const [platform, setPlatform] = useState("all");
  const [storageError, setStorageError] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  useEffect(() => { try { localStorage.setItem(KEY, JSON.stringify(preferences)); setStorageError(false); } catch { setStorageError(true); } }, [preferences]);

  // Só as plataformas que existem no catálogo viram filtro.
  const platforms = useMemo(() => {
    const ids = [...new Set(rooms.map(platformOf).filter((id): id is string => Boolean(id)))];
    return [{ id: "all", label: "Todas" }, ...ids.sort().map((id) => ({ id, label: PLATFORM_LABELS[id] ?? id }))];
  }, [rooms]);
  const visible = selectCollection(rooms, runs, preferences, query, "all").filter((room) => platform === "all" || platformOf(room) === platform);
  const selected = rooms.find((room) => room.id === selectedRoomId) ?? null;
  const selectionHidden = selected && !visible.some((room) => room.id === selected.id);
  const view = bigPicture ? "grid" : preferences.view;
  const toggleFavorite = (id: string) => setPreferences((current) => ({ ...current, favorites: current.favorites.includes(id) ? current.favorites.filter((f) => f !== id) : [...current.favorites, id] }));
  function resetFilters() { setQuery(""); setPlatform("all"); }

  const selectedRuns = selected ? runs.filter((run) => run.gameId === selected.id).sort((a, b) => b.startedAt - a.startedAt) : [];
  const lastRun = selectedRuns[0];
  const recordedMs = selectedRuns.reduce((total, run) => total + Math.max(0, (run.endedAt ?? Date.now()) - run.startedAt), 0);
  const dateFormat = new Intl.DateTimeFormat(locale, { dateStyle: "short", timeStyle: "short" });

  return <div className={`collection-layout ${compact ? "compact" : ""}`}>
    <section className="collection-pane" aria-label={t("Coleção de jogos")}>
      {importOpen && <ImportGamesDialog onClose={() => setImportOpen(false)} />}
      <header className="collection-heading">
        <h1>{t("Minhas coleções")}</h1>
        <button className="secondary-action" onClick={() => setImportOpen(true)}>{t("Importar jogos")}</button>
      </header>

      <div className="collection-toolbar">
        <label className="collection-search">
          <input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("Buscar seus jogos…")} aria-label={t("Buscar jogos")} />
          <span aria-hidden="true">⌕</span>
        </label>
        <Segmented label="Plataforma" value={platform} options={platforms} onChange={setPlatform} />
        <Segmented label="Ordenar por" value={preferences.sort} options={SORTS} onChange={(sort) => setPreferences((current) => ({ ...current, sort }))} />
        {!bigPicture && <div className="collection-view-tools" role="group" aria-label={t("Visualização da coleção")}>
          <button type="button" aria-label={t(preferences.descending ? "Ordenar nomes de A a Z" : "Ordenar nomes de Z a A")} onClick={() => setPreferences((current) => ({ ...current, descending: !current.descending }))}>{preferences.descending ? "Z A ↓" : "A Z ↑"}</button>
          <button type="button" aria-pressed={view === "grid"} aria-label={t("Grade")} onClick={() => setPreferences((current) => ({ ...current, view: "grid" }))}>▦</button>
          <button type="button" aria-pressed={view === "list"} aria-label={t("Lista")} onClick={() => setPreferences((current) => ({ ...current, view: "list" }))}>☰</button>
        </div>}
      </div>

      {selectionHidden && <p className="selection-notice">{selected.title} {t(" continua selecionado no painel de detalhes, mas está fora do filtro atual. ")}<button className="text-action" onClick={resetFilters}>{t("Limpar filtros")}</button></p>}

      <div className={`game-grid ${view === "list" ? "collection-list" : ""}`}>
        {visible.map((room) => {
          const favorite = preferences.favorites.includes(room.id);
          const count = runs.filter((run) => run.gameId === room.id).length;
          const platformId = platformOf(room);
          return <article className={`collection-card ${selectedRoomId === room.id ? "selected" : ""}`} key={room.id}>
            <button type="button" className="game-card" aria-label={t("Selecionar {game}", { game: room.title })} aria-pressed={selectedRoomId === room.id} title={t("Clique duas vezes para abrir a sala")} onClick={() => onSelectRoom(room.id)} onDoubleClick={() => onOpenRoom(room.id)} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); onOpenRoom(room.id); } }}>
              <GameArt room={room} decorative />
              <div className="game-card-info">
                <strong>{room.title}</strong>
                <span className="game-card-meta"><span>{platformId ? t(PLATFORM_LABELS[platformId] ?? platformId) : "—"}</span><span className="supported">✓ {t("Suportado")}</span></span>
                <span className="game-card-runs"><i aria-hidden="true">◇</i>{t("Runs nesta sessão: {count}", { count })}</span>
                {recordingRoomId === room.id && <small className="recording-label">{t("● Gravando run")}</small>}
              </div>
            </button>
            <button className={`favorite-action ${favorite ? "is-favorite" : ""}`} aria-label={t(favorite ? "Remover {game} dos favoritos" : "Adicionar {game} aos favoritos", { game: room.title })} aria-pressed={favorite} onClick={() => toggleFavorite(room.id)}>{favorite ? "★" : "☆"}</button>
          </article>;
        })}
      </div>
      {visible.length === 0 && <div className="collection-empty"><p>{t("Nenhum jogo corresponde aos filtros.")}</p><button className="text-action" onClick={resetFilters}>{t("Mostrar todos os jogos")}</button></div>}
      {storageError && <p role="status" className="muted">{t("Alterações aplicadas nesta sessão; não foi possível salvar as preferências da coleção.")}</p>}

      <footer className="collection-statusbar">
        <span role="status">ⓘ {t("Mostrando {count} de {total} jogos", { count: visible.length, total: rooms.length })}</span>
        <span>{t("Clique duas vezes num jogo (ou Enter) para abrir a sala.")}</span>
      </footer>
    </section>

    {!compact && <aside className="collection-details" aria-label={t("Detalhes do jogo")}>
      {selected ? <>
        <header>
          <h2>{selected.title}</h2>
          <button className={`favorite-action ${preferences.favorites.includes(selected.id) ? "is-favorite" : ""}`} aria-pressed={preferences.favorites.includes(selected.id)} aria-label={t(preferences.favorites.includes(selected.id) ? "Remover {game} dos favoritos" : "Adicionar {game} aos favoritos", { game: selected.title })} onClick={() => toggleFavorite(selected.id)}>{preferences.favorites.includes(selected.id) ? "★" : "☆"}</button>
        </header>
        <ArcadeCabinet room={selected} />
        <dl className="collection-facts two-columns">
          <div><dt>{t("Plataforma")}</dt><dd>{platformOf(selected) ? t(PLATFORM_LABELS[platformOf(selected)!] ?? platformOf(selected)!) : "—"}</dd></div>
          <div><dt>{t("Compatibilidade")}</dt><dd className="supported">✓ {t("Suportado")}</dd></div>
        </dl>
        <dl className="collection-facts">
          <div><dt>{t("Desenvolvedora")}</dt><dd>{selected.developer ?? "—"}</dd></div>
          <div><dt>{t("Publicadora")}</dt><dd>{selected.publisher ?? "—"}</dd></div>
        </dl>
        <div className="collection-actions">
          <button type="button" className="collection-main-action" onClick={() => onOpenRoom(selected.id)}>▷ {t("ABRIR SALA")}</button>
          {onOpenRanking && <button type="button" className="collection-secondary-action" onClick={() => onOpenRanking(selected.id)}>☆ {t("RANKING")}</button>}
        </div>
        <section className="collection-last">
          <h3>{t("ÚLTIMA GAMEPLAY")}</h3>
          <p>{lastRun ? t("{status} · {date}", { status: t(RUN_STATUS[lastRun.status]), date: dateFormat.format(lastRun.startedAt) }) : t("Nenhuma gameplay deste jogo nesta sessão. Abra a sala e inicie pela Steam para gravar.")}</p>
          <dl className="collection-facts">
            <div><dt>{t("Gênero")}</dt><dd>{selected.genre ?? "—"}</dd></div>
            <div><dt>{t("Ano")}</dt><dd>{selected.year ?? "—"}</dd></div>
            <div><dt>{t("Última gameplay gravada")}</dt><dd>{lastRun ? dateFormat.format(lastRun.startedAt) : "—"}</dd></div>
            <div><dt>{t("Tempo de gameplay gravado")}</dt><dd>{selectedRuns.length ? formatDuration(recordedMs) : "—"}</dd></div>
          </dl>
        </section>
      </> : <p className="collection-details-empty">{t("Selecione um jogo para ver os detalhes.")}</p>}
    </aside>}
  </div>;
}
