import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import MappingCanvas from "../../components/canvas/MappingCanvas";
import type { LabGame } from "../../types/game";
import type { MappingBox } from "../../types/mapping";
import type { ProfileVariant } from "../../types/profile";

interface GameWorkspaceProps {
  game: LabGame;
  variant: ProfileVariant;
  boxes: MappingBox[];
  onChangeVariant: (variant: ProfileVariant) => void;
  onChangeBoxes: (boxes: MappingBox[]) => void;
}

interface ExportStatus {
  type: "success" | "error";
  message: string;
}

const VARIANT_OPTIONS: ProfileVariant[] = ["lcd", "crt"];

export default function GameWorkspace({
  game,
  variant,
  boxes,
  onChangeVariant,
  onChangeBoxes,
}: GameWorkspaceProps) {
  const [imageSize, setImageSize] = useState<{ width: number; height: number } | null>(null);
  const [exportStatus, setExportStatus] = useState<ExportStatus | null>(null);
  const [isExporting, setIsExporting] = useState(false);

  useEffect(() => {
    setImageSize(null);
    setExportStatus(null);
  }, [game.id, variant]);

  async function handleExport() {
    if (!imageSize) {
      setExportStatus({
        type: "error",
        message: "Carregue uma screenshot de referência antes de exportar.",
      });
      return;
    }

    setIsExporting(true);
    setExportStatus(null);

    try {
      const outputPath = await invoke<string>("exportar_perfil_jogo", {
        gameId: game.id,
        variant: variant.toUpperCase(),
        boxes,
        imgWidth: imageSize.width,
        imgHeight: imageSize.height,
      });
      setExportStatus({ type: "success", message: `Perfil exportado em ${outputPath}` });
    } catch (error) {
      setExportStatus({
        type: "error",
        message: typeof error === "string" ? error : "Falha ao exportar o perfil.",
      });
    } finally {
      setIsExporting(false);
    }
  }

  return (
    <section className="flex flex-1 flex-col overflow-hidden">
      <header className="flex flex-wrap items-center justify-between gap-4 border-b border-white/5 bg-zinc-950 px-6 py-4">
        <div>
          <h1 className="text-lg font-semibold text-zinc-100">{game.title}</h1>
          <p className="mt-0.5 text-xs text-zinc-500">
            Steam App ID <span className="text-zinc-300">{game.steamAppId}</span> · Perfil{" "}
            <span className="text-zinc-300">{game.profileVersion}</span>
          </p>
        </div>

        <div className="flex items-center gap-3">
          <div className="flex items-center gap-1 rounded-md border border-white/10 bg-zinc-900 p-1">
            {VARIANT_OPTIONS.map((option) => (
              <button
                key={option}
                type="button"
                onClick={() => onChangeVariant(option)}
                className={`rounded px-3 py-1.5 text-xs font-semibold uppercase tracking-wider transition-colors ${
                  variant === option ? "bg-cyan-400/20 text-cyan-300" : "text-zinc-500 hover:text-zinc-300"
                }`}
              >
                Variante {option.toUpperCase()}
              </button>
            ))}
          </div>

          <button
            type="button"
            onClick={handleExport}
            disabled={isExporting}
            className={`rounded-md border px-4 py-1.5 text-xs font-semibold uppercase tracking-wider transition-colors ${
              isExporting
                ? "cursor-not-allowed border-zinc-700 bg-zinc-800/60 text-zinc-500"
                : "border-emerald-400 bg-emerald-400/10 text-emerald-300 hover:bg-emerald-400/20"
            }`}
          >
            {isExporting ? "Exportando..." : "Exportar Perfil"}
          </button>
        </div>
      </header>

      {exportStatus && (
        <div
          className={`border-b px-6 py-2 text-xs ${
            exportStatus.type === "success"
              ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-300"
              : "border-amber-500/30 bg-amber-500/10 text-amber-300"
          }`}
        >
          {exportStatus.message}
        </div>
      )}

      <div className="flex flex-1 overflow-hidden">
        <MappingCanvas
          key={`${game.id}-${variant}`}
          boxes={boxes}
          onChangeBoxes={onChangeBoxes}
          onImageSizeChange={setImageSize}
        />

        <aside className="w-80 flex-shrink-0 overflow-y-auto border-l border-white/5 bg-zinc-950 px-4 py-4">
          <p className="text-xs font-semibold uppercase tracking-[0.2em] text-zinc-500">Regiões (JSON)</p>
          <pre className="mt-3 overflow-x-auto rounded-md border border-white/5 bg-black/40 p-3 text-[11px] leading-relaxed text-emerald-300">
            {JSON.stringify(boxes, null, 2)}
          </pre>
        </aside>
      </div>
    </section>
  );
}
