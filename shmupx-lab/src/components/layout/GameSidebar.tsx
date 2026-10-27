import { useMemo, useState } from "react";
import { IconSearch } from "../ui/Icons";

export interface SidebarGame {
  id: string;
  title: string;
  /** Versão do script de validação registrado; `null` = sem script. */
  scriptVersion: string | null;
}

interface GameSidebarProps {
  games: SidebarGame[];
  selectedGameId: string | null;
  onSelectGame: (gameId: string) => void;
}

/** Seletor de jogo da Validação: lista do catálogo com busca e o status do script. */
export default function GameSidebar({ games, selectedGameId, onSelectGame }: GameSidebarProps) {
  const [query, setQuery] = useState("");
  const visible = useMemo(() => {
    const term = query.trim().toLowerCase();
    return term ? games.filter((game) => `${game.title} ${game.id}`.toLowerCase().includes(term)) : games;
  }, [games, query]);

  return (
    <aside className="flex h-full w-64 flex-shrink-0 flex-col border-r border-white/5 bg-zinc-950/60">
      <div className="px-4 pt-4 pb-3">
        <p className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">Jogos · {games.length}</p>
        <div className="relative mt-2">
          <IconSearch className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-zinc-500" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Buscar jogo"
            aria-label="Buscar jogo"
            className="h-8 w-full rounded-md border border-white/10 bg-zinc-950 pr-2 pl-8 text-xs text-zinc-100 placeholder:text-zinc-600 outline-none focus:border-cyan-400/60"
          />
        </div>
      </div>

      <ul className="flex-1 space-y-0.5 overflow-y-auto px-2 pb-3">
        {visible.map((game) => {
          const selected = game.id === selectedGameId;
          return (
            <li key={game.id}>
              <button
                type="button"
                onClick={() => onSelectGame(game.id)}
                aria-current={selected ? "true" : undefined}
                className={`w-full rounded-md px-3 py-2 text-left transition-colors focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none ${
                  selected ? "bg-cyan-400/10 text-cyan-100 ring-1 ring-cyan-400/30" : "text-zinc-300 hover:bg-white/[0.04]"
                }`}
              >
                <p className="truncate text-sm font-medium">{game.title}</p>
                <p className="mt-0.5 flex items-center gap-1.5 truncate text-[11px] text-zinc-500">
                  <span aria-hidden className={`h-1.5 w-1.5 flex-shrink-0 rounded-full ${game.scriptVersion ? "bg-emerald-400" : "bg-zinc-600"}`} />
                  {game.scriptVersion ? `script ${game.scriptVersion}` : "sem script"}
                </p>
              </button>
            </li>
          );
        })}
        {visible.length === 0 && <li className="px-3 py-6 text-center text-xs text-zinc-500">Nenhum jogo encontrado.</li>}
      </ul>
    </aside>
  );
}
