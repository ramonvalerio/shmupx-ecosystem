import { useI18n } from "../i18n";
import { useEffect, useState } from "react";
import type { GameRoom } from "../types/room";
import type { SessionRun } from "../types/submission";
import { defaultCollection, selectCollection, validateCollection, type CollectionPreferences } from "./collectionState";
import ImportGamesDialog from "./ImportGamesDialog";
import { useGameLauncher } from "./useGameLauncher";
import { GameArt } from "./RoomView";
import { ArcadeCabinet } from "./ArcadeCabinet";
import { useSteamLibrary } from "../data/steamLibrary";
import { usePlatformOwnership, type PlatformTag } from "../data/platforms";
// Scripts de validação que o instalador leva (só os aprovados no Lab): o mesmo arquivo do build.
import scriptsAceitos from "../../src-tauri/scripts-aceitos.json";

/** Suportado: o Client tem o script de validação do jogo, então a run é validada e entra no ranking. */
const SUPPORTED = new Set(scriptsAceitos.map((script) => script.game));
export const isSupported = (room: GameRoom) => SUPPORTED.has(room.id);

const KEY = "shmupx.client.collection.v1";
function readPreferences() { try { return validateCollection(JSON.parse(localStorage.getItem(KEY) ?? "null")); } catch { return defaultCollection(); } }

/**
 * Etiquetas das plataformas em que o jogo roda; azul = o jogador tem o jogo nela (dá para jogar
 * por ela).
 */
function PlatformTags({ tags }: { tags: PlatformTag[] }) {
  const { t } = useI18n();
  if (tags.length === 0) return <span className="play-tags muted">—</span>;
  return (
    <span className="play-tags">
      {tags.map((tag) => (
        <span key={tag.id} className={`play-tag ${tag.owned ? "is-owned" : ""}`} title={t(tag.owned ? "Você tem este jogo no {platform}" : "Roda no {platform}; você ainda não tem o jogo nele", { platform: t(tag.label) })}>
          {t(tag.label)}
        </span>
      ))}
    </span>
  );
}
const PLATFORM_FILTERS = [
  { id: "all", label: "Todas" },
  { id: "emulator", label: "Emulador" },
  { id: "mister", label: "MiSTer" },
  { id: "steam", label: "Steam" },
  { id: "unsupported", label: "Não suportados" },
];

const SORTS: { id: CollectionPreferences["sort"]; label: string }[] = [
  { id: "name", label: "Nome" },
  { id: "developer", label: "Desenvolvedora" },
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

/** Selo de compatibilidade: validado pelo Client ou ainda não (a run não entra no ranking). */
function SupportBadge({ room }: { room: GameRoom }) {
  const { t } = useI18n();
  return isSupported(room)
    ? <span className="supported">✓ {t("Suportado")}</span>
    : <span className="unsupported" title={t("O Client ainda não valida as runs deste jogo: elas não entram no ranking.")}>{t("Não suportado")}</span>;
}

/** MODO TREINO no painel: abre o jogo sem gravar run nem gastar ficha (as mesmas regras da sala). */
function PracticeButton({ room }: { room: GameRoom }) {
  const { t } = useI18n();
  const launcher = useGameLauncher(room);
  return <>
    <button type="button" className="collection-secondary-action practice-action" disabled={launcher.isLaunching} onClick={() => launcher.start(true)} title={t("Abre o jogo sem gravar run nem gastar ficha.")}>
      ⟳ {t(launcher.isLaunching ? "Abrindo o jogo…" : "MODO TREINO")}
    </button>
    {launcher.banners}
    {launcher.dialogs}
  </>;
}

function formatDuration(ms: number) {
  const minutes = Math.max(1, Math.round(ms / 60000));
  return minutes < 60 ? `${minutes}m` : `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
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
  const playable = usePlatformOwnership();
  const [ownedOnly, setOwnedOnly] = useState<"all" | "owned">("all");
  const [refreshing, setRefreshing] = useState(false);
  /** O jogador tem o jogo em alguma plataforma (Steam, emulador ou MiSTer)? */
  const inLibrary = (room: GameRoom) => playable.tags(room).some((tag) => tag.owned);
  async function refreshLibrary() {
    setRefreshing(true);
    try { await steam.refresh(); } finally { setRefreshing(false); }
  }
  useEffect(() => { try { localStorage.setItem(KEY, JSON.stringify(preferences)); setStorageError(false); } catch { setStorageError(true); } }, [preferences]);

  // Só as plataformas que existem no catálogo viram filtro.
  // Filtro por origem: Steam ou emulador (arcade, Neo Geo, consoles… tudo o que não é Steam).
  const platforms = PLATFORM_FILTERS;
  const visible = selectCollection(rooms, runs, preferences, query, "all")
    .filter((room) => platform === "all" || (platform === "unsupported" ? !isSupported(room) : playable.tags(room).some((tag) => tag.id === platform)))
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
      <div className="collection-toolbar">
        <label className="collection-search">
          <input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("Buscar seus jogos…")} aria-label={t("Buscar jogos")} />
          <span aria-hidden="true">⌕</span>
        </label>
        <button type="button" className="collection-import" onClick={() => setImportOpen(true)}>{t("Importar jogos")}</button>
        <Segmented label="Plataforma" value={platform} options={platforms} onChange={setPlatform} />
        {playable.anyLibrary && <Segmented label="Biblioteca" value={ownedOnly} options={[{ id: "all", label: "Todos" }, { id: "owned", label: "Na biblioteca" }]} onChange={setOwnedOnly} />}
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
          return <article className={`collection-card ${selectedRoomId === room.id ? "selected" : ""}`} key={room.id}>
            <button type="button" className="game-card" aria-label={t("Selecionar {game}", { game: room.title })} aria-pressed={selectedRoomId === room.id} title={t("Clique duas vezes para abrir a sala")} onClick={() => onSelectRoom(room.id)} onDoubleClick={() => onOpenRoom(room.id)} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); onOpenRoom(room.id); } }}>
              <GameArt room={room} decorative />
              <div className="game-card-info">
                <strong>{room.title}</strong>
                <span className="game-card-meta"><PlatformTags tags={playable.tags(room)} /><SupportBadge room={room} /></span>
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
          <div><dt>{t("Plataformas")}</dt><dd><PlatformTags tags={playable.tags(selected)} /></dd></div>
          <div><dt>{t("Compatibilidade")}</dt><dd><SupportBadge room={selected} /></dd></div>
        </dl>
        <dl className="collection-facts">
          <div><dt>{t("Desenvolvedora")}</dt><dd>{selected.developer ?? "—"}</dd></div>
          <div><dt>{t("Publicadora")}</dt><dd>{selected.publisher ?? "—"}</dd></div>
        </dl>
        <div className="collection-actions">
          <button type="button" className="collection-main-action" onClick={() => onOpenRoom(selected.id)}>▷ {t("ABRIR SALA")}</button>
          <PracticeButton room={selected} />
          {onOpenRanking && <button type="button" className="collection-secondary-action" onClick={() => onOpenRanking(selected.id)}>☆ {t("RANKING")}</button>}
        </div>
        <section className="collection-last">
          <h3>{t("ÚLTIMA GAMEPLAY")}</h3>
          <p>{lastRun ? t("{status} · {date}", { status: t(RUN_STATUS[lastRun.status]), date: dateFormat.format(lastRun.startedAt) }) : t("Nenhuma gameplay deste jogo nesta sessão. Abra a sala e clique em Iniciar jogo para gravar.")}</p>
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
