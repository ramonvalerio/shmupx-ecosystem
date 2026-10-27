import { useEffect, useMemo, useState } from "react";
import type { CatalogGame } from "../../lib/backend";
import Button from "../../components/ui/Button";
import { IconAlert, IconCheck, IconPlus, IconRefresh, IconSearch } from "../../components/ui/Icons";
import CatalogTable from "./CatalogTable";
import GameEditor from "./GameEditor";

interface CatalogViewProps {
  games: CatalogGame[];
  loading: boolean;
  error: string | null;
  onReload: () => void;
  onSaved: (game: CatalogGame) => void;
}

type StatusFilter = "all" | "published" | "draft";
/** `"new"` = criando; um id = editando esse jogo; `null` = painel fechado. */
type Editing = "new" | string | null;

/** Página do catálogo: lista com busca e filtros; criação e edição num painel lateral. */
export default function CatalogView({ games, loading, error, onReload, onSaved }: CatalogViewProps) {
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<StatusFilter>("all");
  const [editing, setEditing] = useState<Editing>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 4000);
    return () => window.clearTimeout(timer);
  }, [notice]);

  const counts = useMemo(
    () => ({ all: games.length, published: games.filter((g) => g.published).length, draft: games.filter((g) => !g.published).length }),
    [games],
  );

  const visible = useMemo(() => {
    const term = query.trim().toLowerCase();
    return games.filter((game) => {
      if (status === "published" && !game.published) return false;
      if (status === "draft" && game.published) return false;
      if (!term) return true;
      return [game.title, game.id, game.developer, game.publisher].some((field) => field?.toLowerCase().includes(term));
    });
  }, [games, query, status]);

  const editingGame = editing && editing !== "new" ? games.find((game) => game.id === editing) ?? null : null;

  function handleSaved(game: CatalogGame) {
    onSaved(game);
    setEditing(null);
    setNotice(`${game.title} ${game.published ? "publicado" : "salvo como rascunho"}.`);
  }

  const FILTERS: { id: StatusFilter; label: string }[] = [
    { id: "all", label: "Todos" },
    { id: "published", label: "Publicados" },
    { id: "draft", label: "Rascunhos" },
  ];

  return (
    <div className="relative flex min-w-0 flex-1 overflow-hidden">
      <section className="flex min-w-0 flex-1 flex-col">
        <header className="flex flex-wrap items-end justify-between gap-4 px-8 pt-7 pb-5">
          <div>
            <h1 className="text-xl font-semibold text-zinc-100">Catálogo</h1>
            <p className="mt-1 max-w-2xl text-sm text-zinc-500">
              Cada jogo tem uma sala, um RobotX e um validador, todos pelo mesmo ID. Publicado = visível no Client e liberado para runs.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="ghost" onClick={onReload} disabled={loading} title="Recarregar" aria-label="Recarregar">
              <IconRefresh className={loading ? "animate-spin" : ""} />
            </Button>
            <Button variant="primary" onClick={() => setEditing("new")}>
              <IconPlus />
              Novo jogo
            </Button>
          </div>
        </header>

        <div className="flex flex-wrap items-center gap-3 px-8 pb-4">
          <div className="relative w-full max-w-xs">
            <IconSearch className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-zinc-500" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Buscar por título, ID ou desenvolvedora"
              aria-label="Buscar no catálogo"
              className="h-9 w-full rounded-md border border-white/10 bg-zinc-950 pr-3 pl-9 text-sm text-zinc-100 placeholder:text-zinc-600 outline-none focus:border-cyan-400/60 focus:ring-2 focus:ring-cyan-400/15"
            />
          </div>
          <div role="tablist" aria-label="Filtrar por estado" className="flex rounded-md border border-white/10 bg-zinc-950 p-0.5">
            {FILTERS.map((filter) => (
              <button
                key={filter.id}
                type="button"
                role="tab"
                aria-selected={status === filter.id}
                onClick={() => setStatus(filter.id)}
                className={`rounded px-3 py-1.5 text-xs transition-colors ${status === filter.id ? "bg-white/[0.08] font-medium text-zinc-100" : "text-zinc-400 hover:text-zinc-200"}`}
              >
                {filter.label} <span className="text-zinc-500 tabular-nums">{counts[filter.id]}</span>
              </button>
            ))}
          </div>
        </div>

        {error && (
          <p role="alert" className="mx-8 mb-4 flex gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
            <IconAlert className="mt-0.5 flex-shrink-0" />
            {error}
          </p>
        )}

        <div className="mx-8 mb-8 min-h-0 flex-1 overflow-auto rounded-lg border border-white/5 bg-zinc-950/40">
          <CatalogTable
            games={visible}
            loading={loading}
            filtered={games.length > 0}
            selectedId={editing && editing !== "new" ? editing : null}
            onOpen={(game) => setEditing(game.id)}
            onCreate={() => setEditing("new")}
          />
        </div>
      </section>

      {editing !== null && (
        // `key` remonta o editor ao trocar de jogo: o rascunho e as sugestões começam limpos.
        <GameEditor key={editing} game={editingGame} onClose={() => setEditing(null)} onSaved={handleSaved} />
      )}

      {notice && (
        <div role="status" className="pointer-events-none fixed right-6 bottom-6 z-30 flex items-center gap-2 rounded-lg border border-emerald-400/20 bg-zinc-900 px-4 py-3 text-sm text-zinc-100 shadow-xl shadow-black/40">
          <IconCheck className="text-emerald-300" />
          {notice}
        </div>
      )}
    </div>
  );
}
