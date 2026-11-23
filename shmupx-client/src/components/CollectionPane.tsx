import { useI18n } from "../i18n";
import { useEffect, useState } from "react";
import type { GameRoom } from "../types/room";
import type { SessionRun } from "../types/submission";
import { defaultCollection, selectCollection, validateCollection, type CollectionPreferences } from "./collectionState";
import ImportGamesDialog from "./ImportGamesDialog";
import { useGameLauncher } from "./useGameLauncher";
import { useRoomPlay } from "../data/roomPlay";
import PlaySetup from "./PlaySetup";
import RankedPanel, { useRankedLive } from "./RankedPanel";
import { GameArt } from "./RoomView";
import { ArcadeCabinet } from "./ArcadeCabinet";
import { useSteamLibrary } from "../data/steamLibrary";
import { usePlatformOwnership, type PlatformTag } from "../data/platforms";
import { isSupported } from "../data/validation";

export { isSupported };

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
type PlatformFilter = "emulator" | "mister" | "steam" | "soon";
const PLATFORM_FILTERS: { id: PlatformFilter; label: string }[] = [
  { id: "emulator", label: "Emulador" },
  { id: "mister", label: "MiSTer" },
  { id: "steam", label: "Steam" },
  { id: "soon", label: "Coming soon" },
];
type LibraryFilter = "owned" | "missing";
const LIBRARY_FILTERS: { id: LibraryFilter; label: string }[] = [
  { id: "owned", label: "Na biblioteca" },
  { id: "missing", label: "Não tenho ainda" },
];

const SORTS: { id: CollectionPreferences["sort"]; label: string }[] = [
  { id: "name", label: "Nome" },
  { id: "developer", label: "Desenvolvedora" },
  { id: "favorites", label: "Favoritos" },
];

/**
 * Filtro de seleção múltipla. "Todas/Todos" fica marcado com todas as opções marcadas; clicar nele
 * marca todas (ou desmarca todas, se já estavam). Desmarcar uma opção desmarca o "Todas".
 */
function MultiSegmented<T extends string>({ label, allLabel, value, options, onChange }: { label: string; allLabel: string; value: T[]; options: { id: T; label: string }[]; onChange: (ids: T[]) => void }) {
  const { t } = useI18n();
  const all = options.every((option) => value.includes(option.id));
  const toggle = (id: T) => onChange(value.includes(id) ? value.filter((item) => item !== id) : [...value, id]);
  return (
    <div className="collection-segment">
      <span>{t(label)}</span>
      <div role="group" aria-label={t(label)}>
        <button type="button" aria-pressed={all} onClick={() => onChange(all ? [] : options.map((option) => option.id))}>{t(allLabel)}</button>
        {options.map((option) => <button type="button" key={option.id} aria-pressed={value.includes(option.id)} onClick={() => toggle(option.id)}>{t(option.label)}</button>)}
      </div>
    </div>
  );
}

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

/** Jogo ainda sem script de validação: as runs dele ainda não entram no ranking. */
function ComingSoonBadge({ room }: { room: GameRoom }) {
  const { t } = useI18n();
  return isSupported(room) ? null : <span className="coming-soon" title={t("O Client ainda não valida as runs deste jogo: elas não entram no ranking.")}>{t("Coming soon")}</span>;
}

/** MODO TREINO no painel: abre o jogo sem gravar run nem gastar ficha (as mesmas regras da sala). */
/** Jogar pelo painel: modo e plataforma (os mesmos da sala) e PLAY TRAINING, que abre direto. */
/**
 * Jogar pela Coleção: modo e plataforma (os mesmos da sala) e o Play Ranked da sala — a ficha
 * (clique nela para jogar valendo ranking) com as opções de live e de tela cheia.
 */
function PlayPanel({ room, uid }: { room: GameRoom; uid: string | undefined }) {
  const { t } = useI18n();
  const ranked = useRankedLive(room);
  const launcher = useGameLauncher(room, { onLive: (platform) => void ranked.start(platform, launcher.ensureInstalled), recordingWindowed: !ranked.prefs.fullscreen });
  const play = useRoomPlay(room, launcher.platforms, launcher.owns);
  return <section className="collection-play" aria-label={t("Jogar")}>
    <PlaySetup play={play} owns={launcher.owns} disabled={launcher.isLaunching || ranked.liveOn} compact />
    <RankedPanel room={room} uid={uid} launcher={launcher} mode={play.mode} platform={play.platform} ranked={ranked} busy={false} compact />
    {ranked.liveError && <p className="room-banner warning" role="alert">{t(ranked.liveError)}</p>}
    {launcher.banners}
    {launcher.dialogs}
  </section>;
}

/** Versão do jogo: a plataforma original do catálogo (Lab), para mostrar no painel. */
const VERSION_LABEL: Record<string, string> = {
  steam: "PC (Steam)", megadrive: "Mega Drive", neogeo: "Neo Geo", arcade: "Arcade", xbox360: "Xbox 360",
  playstation: "PlayStation", switch: "Nintendo Switch", mister: "MiSTer FPGA", other: "Outra",
};
const versionOf = (room: GameRoom) => room.platform ? VERSION_LABEL[room.platform] ?? room.platform : undefined;


export default function CollectionPane({ uid, rooms, runs, selectedRoomId, recordingRoomId, onSelectRoom, onOpenRoom, compact, bigPicture }: {
  /** Jogador conectado: o saldo de fichas do Play Ranked. */
  uid: string | undefined;
  rooms: GameRoom[]; runs: SessionRun[]; selectedRoomId: string | null; recordingRoomId: string | null;
  onSelectRoom: (id: string) => void;
  /** Abre a sala do jogo numa aba (duplo clique ou Enter no card). */
  onOpenRoom: (id: string) => void;
  compact: boolean; bigPicture: boolean;
}) {
  const { t } = useI18n();
  const [preferences, setPreferences] = useState(readPreferences);
  const [query, setQuery] = useState("");
  const [platformFilter, setPlatformFilter] = useState<PlatformFilter[]>(() => PLATFORM_FILTERS.map((option) => option.id));
  const [storageError, setStorageError] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const steam = useSteamLibrary();
  const playable = usePlatformOwnership();
  const [libraryFilter, setLibraryFilter] = useState<LibraryFilter[]>(() => LIBRARY_FILTERS.map((option) => option.id));
  const [refreshing, setRefreshing] = useState(false);
  /** O jogador tem o jogo em alguma plataforma (Steam, emulador ou MiSTer)? */
  const inLibrary = (room: GameRoom) => playable.tags(room).some((tag) => tag.owned);
  async function refreshLibrary() {
    setRefreshing(true);
    try { await steam.refresh(); } finally { setRefreshing(false); }
  }
  useEffect(() => { try { localStorage.setItem(KEY, JSON.stringify(preferences)); setStorageError(false); } catch { setStorageError(true); } }, [preferences]);

  // Plataformas marcadas: os jogos que rodam em alguma delas. Coming soon soma os jogos ainda sem
  // script de validação (sozinho, mostra só eles). Biblioteca: os que tenho e/ou os que não tenho.
  const platformIds = platformFilter.filter((id): id is Exclude<PlatformFilter, "soon"> => id !== "soon");
  const soon = platformFilter.includes("soon");
  const showPlatform = (room: GameRoom) => platformIds.length === 0
    ? soon && !isSupported(room)
    : (soon || isSupported(room)) && playable.tags(room).some((tag) => (platformIds as string[]).includes(tag.id));
  const showLibrary = (room: GameRoom) => !playable.anyLibrary || libraryFilter.includes(inLibrary(room) ? "owned" : "missing");
  const visible = selectCollection(rooms, runs, preferences, query, "all").filter((room) => showPlatform(room) && showLibrary(room));
  const selected = rooms.find((room) => room.id === selectedRoomId) ?? null;
  const selectionHidden = selected && !visible.some((room) => room.id === selected.id);
  const view = bigPicture ? "grid" : preferences.view;
  const toggleFavorite = (id: string) => setPreferences((current) => ({ ...current, favorites: current.favorites.includes(id) ? current.favorites.filter((f) => f !== id) : [...current.favorites, id] }));
  function resetFilters() {
    setQuery("");
    setPlatformFilter(PLATFORM_FILTERS.map((option) => option.id));
    setLibraryFilter(LIBRARY_FILTERS.map((option) => option.id));
  }


  return <div className={`collection-layout ${compact ? "compact" : ""}`}>
    <section className="collection-pane" aria-label={t("Coleção de jogos")}>
      {importOpen && <ImportGamesDialog rooms={rooms} onClose={() => setImportOpen(false)} />}
      <div className="collection-toolbar">
        <label className="collection-search">
          <input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("Buscar seus jogos…")} aria-label={t("Buscar jogos")} />
          <span aria-hidden="true">⌕</span>
        </label>
        <button type="button" className="collection-import" onClick={() => setImportOpen(true)}>{t("Importar jogos")}</button>
        <MultiSegmented label="Plataforma" allLabel="Todas" value={platformFilter} options={PLATFORM_FILTERS} onChange={setPlatformFilter} />
        {playable.anyLibrary && <MultiSegmented label="Biblioteca" allLabel="Todos" value={libraryFilter} options={LIBRARY_FILTERS} onChange={setLibraryFilter} />}
        <Segmented label="Ordenar por" value={preferences.sort} options={SORTS} onChange={(sort) => setPreferences((current) => ({ ...current, sort }))} />
        {!bigPicture && <div className="collection-segment"><span>{t("Visualização")}</span><div className="collection-view-tools" role="group" aria-label={t("Visualização da coleção")}>
          <button type="button" aria-label={t(preferences.descending ? "Ordenar nomes de A a Z" : "Ordenar nomes de Z a A")} onClick={() => setPreferences((current) => ({ ...current, descending: !current.descending }))}>{preferences.descending ? "Z A ↓" : "A Z ↑"}</button>
          <button type="button" aria-pressed={view === "grid"} aria-label={t("Grade")} onClick={() => setPreferences((current) => ({ ...current, view: "grid" }))}>▦</button>
          <button type="button" aria-pressed={view === "list"} aria-label={t("Lista")} onClick={() => setPreferences((current) => ({ ...current, view: "list" }))}>☰</button>
        </div></div>}
      </div>

      {selectionHidden && <p className="selection-notice">{selected.title} {t(" continua selecionado no painel de detalhes, mas está fora do filtro atual. ")}<button className="text-action" onClick={resetFilters}>{t("Limpar filtros")}</button></p>}

      <div className={`game-grid ${view === "list" ? "collection-list" : ""}`}>
        {visible.map((room) => {
          const favorite = preferences.favorites.includes(room.id);
          return <article className={`collection-card ${selectedRoomId === room.id ? "selected" : ""}`} key={room.id}>
            <button type="button" className="game-card" aria-label={t("Selecionar {game}", { game: room.title })} aria-pressed={selectedRoomId === room.id} title={t("Clique duas vezes para abrir a sala")} onClick={() => onSelectRoom(room.id)} onDoubleClick={() => onOpenRoom(room.id)} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); onOpenRoom(room.id); } }}>
              <GameArt room={room} decorative>{!inLibrary(room) && <span className="not-owned-chip">{t("Não tenho ainda")}</span>}</GameArt>
              <div className="game-card-info">
                <strong>{room.title}</strong>
                <span className="game-card-developer">{room.developer ?? " "}</span>
                <span className="game-card-meta"><PlatformTags tags={playable.tags(room)} /><ComingSoonBadge room={room} /></span>
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
        <ArcadeCabinet room={selected} video />
        <PlayPanel key={selected.id} room={selected} uid={uid} />
        <dl className="collection-facts two-columns">
          <div><dt>{t("Plataformas")}</dt><dd><PlatformTags tags={playable.tags(selected)} /></dd></div>
          <div><dt>{t("Status")}</dt><dd className="collection-status">
            <ComingSoonBadge room={selected} />
            {!inLibrary(selected) && <span className="not-owned">{t("Não tenho ainda")}</span>}
            {isSupported(selected) && inLibrary(selected) && <span>{t("Na biblioteca")}</span>}
          </dd></div>
        </dl>
        <dl className="collection-facts collection-info">
          <div><dt>{t("Versão")}</dt><dd>{versionOf(selected) ? t(versionOf(selected)!) : "—"}</dd></div>
          <div><dt>{t("Desenvolvedora")}</dt><dd>{selected.developer ?? "—"}</dd></div>
          <div><dt>{t("Publicadora")}</dt><dd>{selected.publisher ?? "—"}</dd></div>
          <div><dt>{t("Gênero")}</dt><dd>{selected.genre ?? "—"}</dd></div>
          <div><dt>{t("Ano")}</dt><dd>{selected.year ?? "—"}</dd></div>
        </dl>
      </> : <p className="collection-details-empty">{t("Selecione um jogo para ver os detalhes.")}</p>}
    </aside>}
  </div>;
}
