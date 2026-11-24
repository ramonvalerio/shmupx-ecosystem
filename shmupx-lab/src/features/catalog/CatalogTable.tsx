import type { KeyboardEvent } from "react";
import type { CatalogGame } from "../../lib/backend";
import Badge from "../../components/ui/Badge";
import Button from "../../components/ui/Button";
import { ColumnResizer, useColumnWidths, type ResizableColumn } from "../../components/ui/ColumnResizer";
import { IconPlus } from "../../components/ui/Icons";
import { formatDateTime, playOnLabels, playOnOf } from "./options";

interface CatalogTableProps {
  games: CatalogGame[];
  loading: boolean;
  /** Há jogos no catálogo, mas a busca/filtro não encontrou nenhum. */
  filtered: boolean;
  selectedId: string | null;
  onOpen: (game: CatalogGame) => void;
  onCreate: () => void;
}

const HEAD = "relative truncate px-4 py-2.5 text-[11px] font-semibold tracking-wider text-zinc-500 uppercase";
const CELL = "truncate px-4 py-3";

/** Colunas da grade; a largura de cada uma se ajusta arrastando a borda do cabeçalho. */
const COLUMNS: (ResizableColumn & { label: string; align?: "right" })[] = [
  { key: "game", label: "Jogo", width: 300, min: 140 },
  { key: "platforms", label: "Plataformas", width: 200 },
  { key: "steam", label: "Steam", width: 110 },
  { key: "duration", label: "Run máx.", width: 100, align: "right" },
  { key: "created", label: "Criado em", width: 160 },
  { key: "updated", label: "Atualizado em", width: 160 },
  { key: "status", label: "Estado", width: 120 },
];

export default function CatalogTable({ games, loading, filtered, selectedId, onOpen, onCreate }: CatalogTableProps) {
  const { widths, resize, reset } = useColumnWidths("lab.catalogo.colunas", COLUMNS);

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
    // Layout fixo: cada coluna fica com a largura escolhida; a última coluna, sem largura, ocupa
    // o que sobrar. Colunas largas demais rolam na horizontal.
    <table className="w-full table-fixed text-left text-sm">
      <colgroup>
        {COLUMNS.map((column) => (
          <col key={column.key} style={{ width: widths[column.key] }} />
        ))}
        <col />
      </colgroup>
      <thead className="sticky top-0 z-10 bg-zinc-950/95 backdrop-blur">
        <tr className="border-b border-white/5">
          {COLUMNS.map((column) => (
            <th key={column.key} className={`${HEAD} ${column.align === "right" ? "text-right" : ""}`}>
              {column.label}
              <ColumnResizer
                label={column.label}
                width={widths[column.key]}
                onResize={(width, persist) => resize(column.key, width, persist)}
                onReset={() => reset(column.key)}
              />
            </th>
          ))}
          <th aria-hidden />
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
              <td className={CELL}>
                <p className="truncate font-medium text-zinc-100">{game.title}</p>
                <p className="truncate text-xs text-zinc-500">
                  <span className="font-mono">{game.id}</span>
                  {details && ` · ${details}`}
                </p>
              </td>
              <td className={`${CELL} text-zinc-400`}>{playOnLabels(playOnOf(game)) || "—"}</td>
              <td className={`${CELL} font-mono text-xs text-zinc-400`}>{game.steam_app_id || "—"}</td>
              <td className={`${CELL} text-right text-zinc-400 tabular-nums`}>{Math.round(game.max_duration_segundos / 60)} min</td>
              <td className={`${CELL} text-zinc-400 tabular-nums`}>{formatDateTime(game.created_at)}</td>
              <td className={`${CELL} text-zinc-400 tabular-nums`}>{formatDateTime(game.updated_at)}</td>
              <td className={CELL}>
                <Badge tone={game.published ? "success" : "neutral"}>{game.published ? "Publicado" : "Rascunho"}</Badge>
              </td>
              <td aria-hidden />
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
