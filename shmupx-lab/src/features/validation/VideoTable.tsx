import { useEffect, useState, type KeyboardEvent } from "react";
import { formatDuration, formatScore, queuePosition, type Expected, type ValidationVideo } from "../../lib/validation";
import { LinkActions, PrimaryAction } from "./VideoActions";
import VideoStatus from "./VideoStatus";

interface VideoTableProps {
  videos: ValidationVideo[];
  selectedId: string | null;
  canRun: boolean;
  /** Tempo restante estimado por vídeo (download/análise em andamento). */
  remaining: Map<string, string>;
  onOpen: (video: ValidationVideo) => void;
  onRun: (video: ValidationVideo) => void;
  onCancel: (video: ValidationVideo) => void;
  onDownloadAgain: (video: ValidationVideo) => void;
  onSaveExpected: (video: ValidationVideo, expected: Expected) => Promise<void>;
}

const HEAD = "px-4 py-2.5 text-[11px] font-semibold tracking-wider text-zinc-500 uppercase whitespace-nowrap";

/** Campo do placar esperado editável na própria linha: salva ao sair do campo ou no Enter. */
function ExpectedScoreInput({ video, onSave }: { video: ValidationVideo; onSave: (expected: Expected) => Promise<void> }) {
  const stored = video.gabarito.placar === null ? "" : String(video.gabarito.placar);
  const [value, setValue] = useState(stored);
  const [invalid, setInvalid] = useState(false);
  useEffect(() => setValue(stored), [stored]);

  async function commit() {
    const text = value.replace(/[.\s]/g, "").trim();
    if (text === stored) return;
    const placar = text === "" ? null : Number(text);
    if (placar !== null && (!Number.isInteger(placar) || placar < 0)) {
      setInvalid(true);
      return;
    }
    setInvalid(false);
    await onSave({ ...video.gabarito, placar });
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Enter") event.currentTarget.blur();
    if (event.key === "Escape") {
      setValue(stored);
      setInvalid(false);
      event.currentTarget.blur();
    }
  }

  return (
    <input
      value={value}
      inputMode="numeric"
      placeholder="informar"
      aria-label={`Placar esperado de ${video.titulo}`}
      aria-invalid={invalid}
      onClick={(event) => event.stopPropagation()}
      onChange={(event) => setValue(event.target.value)}
      onBlur={() => void commit()}
      onKeyDown={onKeyDown}
      className={`h-8 w-32 rounded-md border bg-zinc-950 px-2 text-right font-mono text-xs text-zinc-100 tabular-nums placeholder:font-sans placeholder:text-zinc-600 outline-none focus:border-cyan-400/60 ${invalid ? "border-red-400/60" : "border-white/10"}`}
    />
  );
}

export default function VideoTable({ videos, selectedId, canRun, remaining, onOpen, onRun, onCancel, onDownloadAgain, onSaveExpected }: VideoTableProps) {
  const onRowKey = (event: KeyboardEvent, video: ValidationVideo) => {
    if (event.target !== event.currentTarget) return;
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onOpen(video);
    }
  };

  return (
    <table className="w-full table-fixed text-left text-sm">
      {/* O vídeo fica com o espaço que sobra; as demais colunas têm largura fixa. */}
      <colgroup>
        <col />
        <col className="w-40" />
        <col className="w-32" />
        <col className="w-28" />
        <col className="w-60" />
        <col className="w-32" />
      </colgroup>
      <thead className="sticky top-0 z-10 bg-zinc-950/95 backdrop-blur">
        <tr className="border-b border-white/5">
          <th className={HEAD}>Vídeo</th>
          <th className={`${HEAD} text-right`}>Placar esperado</th>
          <th className={`${HEAD} text-right`}>Placar lido</th>
          <th className={HEAD}>Vidas · Cont.</th>
          <th className={HEAD}>Situação</th>
          <th className={HEAD}><span className="sr-only">Ações</span></th>
        </tr>
      </thead>
      <tbody>
        {videos.map((video) => {
          const result = video.resultado;
          const selected = video.id === selectedId;
          const mismatch = video.acertou === false && result && !result.erro;
          return (
            <tr
              key={video.id}
              tabIndex={0}
              aria-selected={selected}
              onClick={() => onOpen(video)}
              onKeyDown={(event) => onRowKey(event, video)}
              className={`cursor-pointer border-b border-white/5 transition-colors outline-none focus-visible:bg-white/[0.05] ${selected ? "bg-cyan-400/[0.06]" : "hover:bg-white/[0.03]"}`}
            >
              <td className="px-4 py-3">
                <p className="truncate font-medium text-zinc-100" title={video.titulo}>{video.titulo}</p>
                <p className="flex items-center gap-1 text-xs text-zinc-500">
                  <span className="truncate">
                    {[video.origem.tipo === "youtube" ? "YouTube" : "Arquivo local", formatDuration(video.duracao_segundos)].filter(Boolean).join(" · ")}
                  </span>
                  {video.origem.tipo === "youtube" && <LinkActions url={video.origem.url} />}
                </p>
              </td>
              <td className="px-4 py-2 text-right">
                <ExpectedScoreInput video={video} onSave={(expected) => onSaveExpected(video, expected)} />
              </td>
              <td className={`px-4 py-3 text-right font-mono text-xs whitespace-nowrap tabular-nums ${mismatch ? "text-red-300" : "text-zinc-300"}`}>
                {formatScore(result?.placar)}
              </td>
              <td className="px-4 py-3 text-xs whitespace-nowrap text-zinc-400 tabular-nums">
                {result && !result.erro ? `${result.vidas ?? "—"} · ${result.continues ?? "—"}` : "—"}
              </td>
              <td className="px-4 py-3"><VideoStatus video={video} remaining={remaining.get(video.id)} queue={queuePosition(videos, video)} /></td>
              <td className="px-4 py-2 text-right">
                <PrimaryAction
                  video={video}
                  canRun={canRun}
                  onRun={() => onRun(video)}
                  onCancel={() => onCancel(video)}
                  onDownloadAgain={() => onDownloadAgain(video)}
                />
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
