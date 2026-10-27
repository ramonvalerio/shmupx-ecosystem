import { useI18n } from "../i18n";
import { useEffect, useState } from "react";
import type { GameRoom } from "../types/room";
import type { SessionRun } from "../types/submission";
import { defaultCollection, selectCollection, validateCollection } from "./collectionState";
import ImportGamesDialog from "./ImportGamesDialog";

const KEY = "shmupx.client.collection.v1";
function readPreferences() { try { return validateCollection(JSON.parse(localStorage.getItem(KEY) ?? "null")); } catch { return defaultCollection(); } }

export default function CollectionPane({ rooms, runs, selectedRoomId, recordingRoomId, onSelectRoom, onOpenRoom, compact, bigPicture }: {
  rooms: GameRoom[]; runs: SessionRun[]; selectedRoomId: string | null; recordingRoomId: string | null;
  onSelectRoom: (id: string) => void;
  /** Abre a sala do jogo numa aba (duplo clique ou Enter no card). */
  onOpenRoom: (id: string) => void;
  compact: boolean; bigPicture: boolean;
}) {
  const { t } = useI18n();
  const [preferences, setPreferences] = useState(readPreferences);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const [storageError, setStorageError] = useState(false);
  const [importOpen,setImportOpen] = useState(false);
  const visible = selectCollection(rooms, runs, preferences, query, filter);
  const selected = rooms.find((room) => room.id === selectedRoomId);
  const selectionHidden = selected && !visible.some((room) => room.id === selected.id);
  useEffect(() => { try { localStorage.setItem(KEY, JSON.stringify(preferences)); setStorageError(false); } catch { setStorageError(true); } }, [preferences]);
  function resetFilters() { setQuery(""); setFilter("all"); }
  const view = bigPicture ? "grid" : preferences.view;
  return <section className={`collection-pane ${compact ? "compact" : ""}`} aria-label={t("Coleção de jogos")}>
    {importOpen && <ImportGamesDialog onClose={()=>setImportOpen(false)} />}
    <header className="collection-heading"><div><p className="eyebrow">{t("SUA BIBLIOTECA")}</p><h1>{compact ? t("Salas") : t("Coleção")}</h1></div><span className="game-count">{rooms.length} {t(" jogos")}</span></header>
    <div className="collection-import-toolbar"><button className="secondary-action" onClick={()=>setImportOpen(true)}>{t("Importar jogos")}</button></div>
    <label className="collection-search"><span aria-hidden="true">⌕</span><input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("Buscar jogos…")} aria-label={t("Buscar jogos")} /></label>
    <div className="collection-controls">
      <label>{t("Exibir")}<select value={filter} onChange={(event) => setFilter(event.target.value)}><option value="all">{t("Todos os jogos")}</option><option value="favorites">{t("Favoritos")}</option><option value="recorded">{t("Com runs nesta sessão")}</option></select></label>
      <label>{t("Ordenar por")}<select value={preferences.sort} onChange={(event) => setPreferences((current) => ({ ...current, sort: event.target.value as typeof current.sort }))}><option value="name">{t("Nome")}</option><option value="favorites">{t("Favoritos primeiro")}</option><option value="recorded">{t("Gravação mais recente")}</option></select></label>
    </div>
    <div className="collection-filter"><span>{t("Steam · demonstração")}</span><span role="status">{visible.length} {t(" de ")}{rooms.length} {t(" jogos")}</span></div>
    {!bigPicture && <div className="collection-view-tools" role="group" aria-label={t("Visualização da coleção")}><button aria-label={t(preferences.descending ? "Ordenar nomes de A a Z" : "Ordenar nomes de Z a A")} onClick={() => setPreferences((current) => ({ ...current, descending: !current.descending }))}>{preferences.descending ? t("Z–A ↓") : t("A–Z ↑")}</button><button aria-pressed={view === "grid"} onClick={() => setPreferences((current) => ({ ...current, view: "grid" }))}>{t("Grade")}</button><button aria-pressed={view === "list"} onClick={() => setPreferences((current) => ({ ...current, view: "list" }))}>{t("Lista")}</button></div>}
    {selectionHidden && <p className="selection-notice">{selected.title} {t(" continua selecionado no painel de detalhes, mas está fora do filtro atual. ")}<button className="text-action" onClick={resetFilters}>{t("Limpar filtros")}</button></p>}
    <div className={`game-grid ${view === "list" ? "collection-list" : ""}`}>
      {visible.map((room) => {
        const favorite = preferences.favorites.includes(room.id);
        const count = runs.filter((run) => run.gameId === room.id).length;
        return <article className={`collection-card ${selectedRoomId === room.id ? "selected" : ""}`} key={room.id}>
          <button type="button" className="game-card" aria-label={t("Selecionar {game}", {game:room.title})} aria-pressed={selectedRoomId === room.id} title={t("Clique duas vezes para abrir a sala")} onClick={() => onSelectRoom(room.id)} onDoubleClick={() => onOpenRoom(room.id)} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); onOpenRoom(room.id); } }}>
            <div className={`game-art art-${room.id}`}>{room.id === "dodonpachi" ? <img src="/img/dodonpachi.png" alt="" /> : <span>{room.title}</span>}<span className="platform-tag">{t("STEAM")}</span></div>
            <div className="game-card-info"><strong>{room.title}</strong><span><i className="status-dot" />{room.playersOnline} {t(" na sala · demo")}</span>{count > 0 && <small>{count} {count === 1 ? t("run nesta sessão") : t("runs nesta sessão")}</small>}{recordingRoomId === room.id && <small className="recording-label">{t("● Gravando run")}</small>}</div>
          </button>
          <button className={`favorite-action ${favorite ? "is-favorite" : ""}`} aria-label={t(favorite ? "Remover {game} dos favoritos" : "Adicionar {game} aos favoritos", {game:room.title})} aria-pressed={favorite} onClick={() => setPreferences((current) => ({ ...current, favorites: current.favorites.includes(room.id) ? current.favorites.filter((id) => id !== room.id) : [...current.favorites, room.id] }))}>{favorite ? t("★") : t("☆")}</button>
        </article>;
      })}
    </div>
    {visible.length === 0 && <div className="collection-empty"><p>{filter === "favorites" && !query ? t("Você ainda não tem jogos favoritos. Marque uma estrela para começar.") : filter === "recorded" && !query ? t("Nenhum jogo tem runs observadas nesta sessão.") : t("Nenhum jogo corresponde aos filtros.")}</p><button className="text-action" onClick={resetFilters}>{t("Mostrar todos os jogos")}</button></div>}
    {storageError && <p role="status" className="muted">{t("Alterações aplicadas nesta sessão; não foi possível salvar as preferências da coleção.")}</p>}
    <p className="collection-note">{t("Clique duas vezes num jogo (ou Enter) para abrir a sala: cada sala abre numa aba, e o botão Coleção volta para cá. A estrela marca seus favoritos neste dispositivo.")}</p>
  </section>;
}
