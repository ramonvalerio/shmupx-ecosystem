import type { KeyboardEvent } from "react";
import type { CatalogGame } from "../../lib/backend";
import Badge from "../../components/ui/Badge";
import Button from "../../components/ui/Button";
import { IconPlus } from "../../components/ui/Icons";
import { platformLabel } from "./options";

interface CatalogTableProps {
  games: CatalogGame[];
  loading: boolean;
  /** Há jogos no catálogo, mas a busca/filtro não encontrou nenhum. */
  filtered: boolean;
  selectedId: string | null;
  onOpen: (game: CatalogGame) => void;
  onCreate: () => void;
}

const HEAD = "px-4 py-2.5 text-[11px] font-semibold tracking-wider text-zinc-500 uppercase";

export default function CatalogTable({ games, loading, filtered, selectedId, onOpen, onCreate }: CatalogTableProps) {
  if (loading && games.length === 0) {
    return (
      <div className="space-y-px" aria-busy="true" aria-label="Carregando o catálogo">
        {[0, 1, 2, 3].map((row) => (
          <div key={row} className="flex items-center gap-4 px-4 py-4">
            <div className="h-3 w-56 animate-pulse rounded bg-white/5" />
            <div className="h-3 w-24 animate-pulse rounded bg-white/5" />
            <div className="ml-auto h-3 w-16 animate-pulse rounded bg-white/5" />
          </div>
        ))}
      </div>
    );
  }

  if (games.length === 0) {
    return (
      <div className="flex h-full min-h-64 flex-col items-center justify-center gap-3 px-6 py-16 text-center">
        {filtered ? (
          <p className="text-sm text-zinc-400">Nenhum jogo corresponde à busca.</p>
        ) : (
          <>
            <p className="text-sm font-medium text-zinc-200">O catálogo está vazio</p>
            <p className="max-w-sm text-xs text-zinc-500">Cadastre o primeiro jogo. Ele só aparece no Client depois de publicado.</p>
            <Button variant="primary" size="sm" onClick={onCreate} className="mt-2">
              <IconPlus />
              Novo jogo
            </Button>
          </>
        )}
      </div>
    );
  }

  const onRowKey = (event: KeyboardEvent, game: CatalogGame) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onOpen(game);
    }
  };

  return (
    <table className="w-full text-left text-sm">
      <thead className="sticky top-0 z-10 bg-zinc-950/95 backdrop-blur">
        <tr className="border-b border-white/5">
          <th className={HEAD}>Jogo</th>
          <th className={HEAD}>Plataforma</th>
          <th className={HEAD}>Steam</th>
          <th className={`${HEAD} text-right`}>Run máx.</th>
          <th className={HEAD}>Estado</th>
        </tr>
      </thead>
      <tbody>
        {games.map((game) => {
          const selected = game.id === selectedId;
          const details = [game.developer, game.year].filter(Boolean).join(" · ");
          return (
            <tr
              key={game.id}
              tabIndex={0}
              aria-selected={selected}
              onClick={() => onOpen(game)}
              onKeyDown={(event) => onRowKey(event, game)}
              className={`cursor-pointer border-b border-white/5 transition-colors outline-none focus-visible:bg-white/[0.05] ${selected ? "bg-cyan-400/[0.06]" : "hover:bg-white/[0.03]"}`}
            >
              <td className="max-w-0 px-4 py-3">
                <p className="truncate font-medium text-zinc-100">{game.title}</p>
                <p className="truncate text-xs text-zinc-500">
                  <span className="font-mono">{game.id}</span>
                  {details && ` · ${details}`}
                </p>
              </td>
              <td className="px-4 py-3 whitespace-nowrap text-zinc-400">{platformLabel(game.platform) || "—"}</td>
              <td className="px-4 py-3 font-mono text-xs whitespace-nowrap text-zinc-400">{game.steam_app_id || "—"}</td>
              <td className="px-4 py-3 text-right whitespace-nowrap text-zinc-400 tabular-nums">{Math.round(game.max_duration_segundos / 60)} min</td>
              <td className="px-4 py-3">
                <Badge tone={game.published ? "success" : "neutral"}>{game.published ? "Publicado" : "Rascunho"}</Badge>
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
