import { useI18n } from "../i18n";
import { useEffect, useState } from "react";
import type { GameRoom } from "../types/room";
import type { SessionRun } from "../types/submission";
import { defaultCollection, selectCollection, validateCollection, type CollectionPreferences } from "./collectionState";
import ImportGamesDialog from "./ImportGamesDialog";
import { GameArt } from "./RoomView";
import { useGameArt } from "../data/gameArt";
import { useSteamLibrary } from "../data/steamLibrary";

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

/** Grupo do filtro de plataforma: Steam ou emulador. */
export function platformGroup(room: GameRoom): "steam" | "emulator" {
  return platformOf(room) === "steam" ? "steam" : "emulator";
}
const PLATFORM_FILTERS = [
  { id: "all", label: "Todas" },
  { id: "emulator", label: "Emulador" },
  { id: "steam", label: "Steam" },
];

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
  const art = useGameArt(room);
  return (
    <svg className="arcade-cabinet" viewBox="0 0 320 300" role="img" aria-label={room.title}>
      {/* Inspirado no Sega Astro City: corpo claro de cantos redondos, marquise iluminada (sem texto),
          monitor fundo em moldura escura, painel preto largo e porta da ficha embaixo. */}
      <defs>
        <clipPath id="cabinet-screen"><rect x="58" y="48" width="204" height="118" rx="5" /></clipPath>
        <linearGradient id="astro-body" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stopColor="#b9c0c6" /><stop offset=".18" stopColor="#e9ecef" /><stop offset=".82" stopColor="#e9ecef" /><stop offset="1" stopColor="#b9c0c6" /></linearGradient>
        <linearGradient id="astro-marquee" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stopColor="#e9fdff" /><stop offset="1" stopColor="#7fe7ee" /></linearGradient>
        <linearGradient id="astro-glass" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor="#ffffff" stopOpacity=".1" /><stop offset=".45" stopColor="#ffffff" stopOpacity="0" /></linearGradient>
      </defs>
      {/* Corpo de cima: topo bem arredondado */}
      <path d="M46 4 H274 Q300 4 301 30 L304 182 H16 L19 30 Q20 4 46 4 Z" fill="url(#astro-body)" stroke="#7d878e" strokeWidth="1.5" />
      {/* Marquise iluminada, sem nome do jogo */}
      <rect x="40" y="12" width="240" height="22" rx="8" fill="url(#astro-marquee)" stroke="#5fb8c0" strokeWidth="1" />
      <rect x="40" y="12" width="240" height="22" rx="8" fill="none" stroke="#ffffff" strokeOpacity=".6" strokeWidth="1" transform="translate(0 1)" />
      {/* Moldura escura e funda do monitor, com os alto-falantes nas laterais */}
      <path d="M30 42 Q30 38 34 38 H286 Q290 38 290 42 L292 176 H28 Z" fill="#14181b" />
      <g fill="#3a4248"><circle cx="34" cy="60" r="1.5" /><circle cx="34" cy="66" r="1.5" /><circle cx="34" cy="72" r="1.5" /><circle cx="34" cy="78" r="1.5" /><circle cx="34" cy="84" r="1.5" /><circle cx="40" cy="60" r="1.5" /><circle cx="40" cy="66" r="1.5" /><circle cx="40" cy="72" r="1.5" /><circle cx="40" cy="78" r="1.5" /><circle cx="40" cy="84" r="1.5" /><circle cx="46" cy="60" r="1.5" /><circle cx="46" cy="66" r="1.5" /><circle cx="46" cy="72" r="1.5" /><circle cx="46" cy="78" r="1.5" /><circle cx="46" cy="84" r="1.5" /><circle cx="52" cy="60" r="1.5" /><circle cx="52" cy="66" r="1.5" /><circle cx="52" cy="72" r="1.5" /><circle cx="52" cy="78" r="1.5" /><circle cx="52" cy="84" r="1.5" /><circle cx="262" cy="60" r="1.5" /><circle cx="262" cy="66" r="1.5" /><circle cx="262" cy="72" r="1.5" /><circle cx="262" cy="78" r="1.5" /><circle cx="262" cy="84" r="1.5" /><circle cx="268" cy="60" r="1.5" /><circle cx="268" cy="66" r="1.5" /><circle cx="268" cy="72" r="1.5" /><circle cx="268" cy="78" r="1.5" /><circle cx="268" cy="84" r="1.5" /><circle cx="274" cy="60" r="1.5" /><circle cx="274" cy="66" r="1.5" /><circle cx="274" cy="72" r="1.5" /><circle cx="274" cy="78" r="1.5" /><circle cx="274" cy="84" r="1.5" /><circle cx="280" cy="60" r="1.5" /><circle cx="280" cy="66" r="1.5" /><circle cx="280" cy="72" r="1.5" /><circle cx="280" cy="78" r="1.5" /><circle cx="280" cy="84" r="1.5" /></g>
      <rect x="52" y="42" width="216" height="130" rx="10" fill="#050708" stroke="#2a3136" strokeWidth="2" />
      <rect x="58" y="48" width="204" height="118" rx="5" fill="#000" />
      <image className={art.grayscale ? "is-not-owned" : ""} href={art.src} x="58" y="48" width="204" height="118" preserveAspectRatio="xMidYMid meet" clipPath="url(#cabinet-screen)" onError={art.onError} />
      <rect x="58" y="48" width="204" height="118" rx="5" fill="url(#astro-glass)" pointerEvents="none" />
      {/* Painel de controle preto, largo e projetado à frente */}
      <path d="M12 182 H308 Q316 182 316 191 L317 207 Q317 216 308 216 H12 Q3 216 3 207 L4 191 Q4 182 12 182 Z" fill="#16191c" stroke="#7d878e" strokeWidth="1.5" />
      <path d="M8 186 H312" stroke="#ffffff" strokeOpacity=".12" strokeWidth="1" />
      <line x1="111" y1="203" x2="111" y2="191" stroke="#d7e4e8" strokeWidth="3" strokeLinecap="round" />
      <circle cx="111" cy="189" r="6" fill="#fd4851" />
      <circle cx="149" cy="199" r="4.5" fill="#fd4851" /><circle cx="165" cy="199" r="4.5" fill="#fd4851" /><circle cx="181" cy="199" r="4.5" fill="#fd4851" />
      <rect x="197" y="195" width="18" height="8" rx="4" fill="#2f7bff" />
      <text x="206" y="210" textAnchor="middle" className="arcade-cabinet-label">START</text>
      {/* Corpo de baixo claro, levemente afunilado, com cantos redondos, porta da ficha e rodapé escuro */}
      <path d="M24 216 H296 L288 280 Q287 288 279 288 H41 Q33 288 32 280 Z" fill="url(#astro-body)" stroke="#7d878e" strokeWidth="1.5" />
      <rect x="134" y="228" width="52" height="40" rx="4" fill="#2a2f33" stroke="#5c656b" strokeWidth="1.2" />
      <rect x="146" y="235" width="7" height="16" rx="1.5" fill="#fd4851" />
      <rect x="167" y="235" width="7" height="16" rx="1.5" fill="#fd4851" />
      <rect x="148" y="257" width="24" height="4" rx="1" fill="#14181b" />
      <rect x="36" y="288" width="248" height="8" rx="3" fill="#14181b" />
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
  const steam = useSteamLibrary();
  const [ownedOnly, setOwnedOnly] = useState<"all" | "owned">("all");
  const [refreshing, setRefreshing] = useState(false);
  /** Na biblioteca importada? `null` = biblioteca não importada ou jogo fora da Steam. */
  const inLibrary = (room: GameRoom) => (steam.library && room.steamAppId > 0 ? steam.owns(room.steamAppId) : null);
  const libraryLabel = (room: GameRoom) => {
    const owned = inLibrary(room);
    return owned === null ? null : owned ? t("Na sua biblioteca") : t("Fora da sua biblioteca");
  };
  async function refreshLibrary() {
    setRefreshing(true);
    try { await steam.refresh(); } finally { setRefreshing(false); }
  }
  useEffect(() => { try { localStorage.setItem(KEY, JSON.stringify(preferences)); setStorageError(false); } catch { setStorageError(true); } }, [preferences]);

  // Só as plataformas que existem no catálogo viram filtro.
  // Filtro por origem: Steam ou emulador (arcade, Neo Geo, consoles… tudo o que não é Steam).
  const platforms = PLATFORM_FILTERS;
  const visible = selectCollection(rooms, runs, preferences, query, "all")
    .filter((room) => platform === "all" || platformGroup(room) === platform)
    .filter((room) => ownedOnly === "all" || inLibrary(room) === true);
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
      {importOpen && <ImportGamesDialog rooms={rooms} onClose={() => setImportOpen(false)} />}
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
        {steam.library && <Segmented label="Biblioteca" value={ownedOnly} options={[{ id: "all", label: "Todos" }, { id: "owned", label: "Na biblioteca" }]} onChange={setOwnedOnly} />}
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
                <span className="game-card-meta"><span className={inLibrary(room) === false ? "not-owned" : ""}>{libraryLabel(room) ?? (platformId ? t(PLATFORM_LABELS[platformId] ?? platformId) : "—")}</span><span className="supported">✓ {t("Suportado")}</span></span>
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
        {steam.library
          ? <button type="button" className="text-action" disabled={refreshing} onClick={() => void refreshLibrary()}>↻ {t(refreshing ? "Atualizando…" : "Atualizar biblioteca")}</button>
          : <button type="button" className="text-action" onClick={() => setImportOpen(true)}>{t("Importar jogos da Steam")}</button>}
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
          <div><dt>{t("Plataforma")}</dt><dd>{platformOf(selected) ? t(PLATFORM_LABELS[platformOf(selected)!] ?? platformOf(selected)!) : "—"}{libraryLabel(selected) && <small className={inLibrary(selected) ? "" : "not-owned"}>{libraryLabel(selected)}</small>}</dd></div>
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
