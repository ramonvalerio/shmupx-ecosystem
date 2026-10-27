import type { LabGame } from "../../types/game";

export type LabView = "catalog" | "mapping";

interface GameSidebarProps {
  games: LabGame[];
  selectedGameId: string | null;
  onSelectGame: (gameId: string) => void;
  view: LabView;
  onChangeView: (view: LabView) => void;
  accountEmail: string;
  onSignOut: () => void;
}

const VIEWS: { id: LabView; label: string }[] = [
  { id: "catalog", label: "Catálogo" },
  { id: "mapping", label: "Mapeamento" },
];

export default function GameSidebar({ games, selectedGameId, onSelectGame, view, onChangeView, accountEmail, onSignOut }: GameSidebarProps) {
  return (
    <aside className="flex h-full w-72 flex-shrink-0 flex-col border-r border-white/5 bg-zinc-950">
      <div className="border-b border-white/5 px-5 py-5">
        <p className="text-xs font-semibold uppercase tracking-[0.25em] text-cyan-400">ShmupX Lab</p>
        <nav className="mt-3 flex gap-1" aria-label="Seções do Lab">
          {VIEWS.map(({ id, label }) => (
            <button
              key={id}
              type="button"
              onClick={() => onChangeView(id)}
              aria-current={view === id ? "page" : undefined}
              className={`flex-1 rounded-md border px-2 py-1.5 text-xs transition-colors ${view === id ? "border-cyan-400/50 bg-cyan-400/10 text-cyan-200" : "border-transparent text-zinc-400 hover:bg-zinc-900"}`}
            >
              {label}
            </button>
          ))}
        </nav>
        <h2 className="mt-4 text-xs uppercase tracking-[0.2em] text-zinc-500">Jogos Cadastrados</h2>
      </div>

      <ul className="flex-1 space-y-1 overflow-y-auto px-3 py-4">
        {games.map((game) => {
          const isSelected = game.id === selectedGameId;
          return (
            <li key={game.id}>
              <button
                type="button"
                onClick={() => onSelectGame(game.id)}
                className={`w-full rounded-md border px-3 py-2.5 text-left transition-colors ${
                  isSelected
                    ? "border-cyan-400/50 bg-cyan-400/10 text-cyan-200"
                    : "border-transparent text-zinc-300 hover:bg-zinc-900"
                }`}
              >
                <p className="text-sm font-medium">{game.title}</p>
                <p className="mt-0.5 text-[11px] text-zinc-500">
                  app_id {game.steamAppId} · {game.profileVersion}
                </p>
              </button>
            </li>
          );
        })}
      </ul>

      <div className="border-t border-white/5 px-5 py-4">
        <p className="truncate text-xs text-zinc-400" title={accountEmail}>{accountEmail}</p>
        <button type="button" onClick={onSignOut} className="mt-2 text-xs text-zinc-500 underline hover:text-zinc-300">Sair</button>
      </div>
    </aside>
  );
}
