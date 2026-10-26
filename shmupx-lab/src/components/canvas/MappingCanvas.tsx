import { useEffect, useRef, useState } from "react";
import type { ChangeEvent, MouseEvent as ReactMouseEvent, SyntheticEvent } from "react";
import type { MappingBox, MappingRegionKey } from "../../types/mapping";

interface MappingCanvasProps {
  boxes: MappingBox[];
  onChangeBoxes: (boxes: MappingBox[]) => void;
  /** Avisa o workspace sobre as dimensões reais da imagem carregada (necessárias para a
   * normalização 0..1 feita no Rust ao exportar o perfil). `null` quando não há imagem. */
  onImageSizeChange: (size: { width: number; height: number } | null) => void;
}

interface DragState {
  key: MappingRegionKey;
  mode: "move" | "resize";
  startClientX: number;
  startClientY: number;
  startBox: MappingBox;
}

/** Largura máxima de exibição; a imagem real pode ser bem maior (ex.: 1920×1080). */
const MAX_DISPLAY_WIDTH = 960;
const MIN_BOX_SIZE = 16;

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), Math.max(min, max));
}

/**
 * Canvas de mapeamento (Spec 001): recebe uma screenshot de referência e permite
 * arrastar/redimensionar as bounding boxes obrigatórias por cima dela. As coordenadas
 * ficam sempre no espaço de pixels *naturais* da imagem (não da exibição em tela),
 * convertidas por `scale` apenas para desenhar — é isso que garante precisão quando a
 * imagem é maior que a área visível.
 */
export default function MappingCanvas({ boxes, onChangeBoxes, onImageSizeChange }: MappingCanvasProps) {
  const [imageUrl, setImageUrl] = useState<string | null>(null);
  const [naturalSize, setNaturalSize] = useState<{ width: number; height: number } | null>(null);
  const [activeDrag, setActiveDrag] = useState<DragState | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const scale = naturalSize ? Math.min(1, MAX_DISPLAY_WIDTH / naturalSize.width) : 1;

  useEffect(() => {
    return () => {
      if (imageUrl) URL.revokeObjectURL(imageUrl);
    };
  }, [imageUrl]);

  useEffect(() => {
    onImageSizeChange(naturalSize);
  }, [naturalSize, onImageSizeChange]);

  useEffect(() => {
    if (!activeDrag || !naturalSize) return;

    function handleMouseMove(event: MouseEvent) {
      if (!activeDrag || !naturalSize) return;
      const deltaX = (event.clientX - activeDrag.startClientX) / scale;
      const deltaY = (event.clientY - activeDrag.startClientY) / scale;

      onChangeBoxes(
        boxes.map((box) => {
          if (box.key !== activeDrag.key) return box;

          if (activeDrag.mode === "move") {
            return {
              ...box,
              x: clamp(activeDrag.startBox.x + deltaX, 0, naturalSize.width - box.width),
              y: clamp(activeDrag.startBox.y + deltaY, 0, naturalSize.height - box.height),
            };
          }

          return {
            ...box,
            width: clamp(activeDrag.startBox.width + deltaX, MIN_BOX_SIZE, naturalSize.width - box.x),
            height: clamp(
              activeDrag.startBox.height + deltaY,
              MIN_BOX_SIZE,
              naturalSize.height - box.y,
            ),
          };
        }),
      );
    }

    function handleMouseUp() {
      setActiveDrag(null);
    }

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [activeDrag, boxes, naturalSize, onChangeBoxes, scale]);

  function handleFileChange(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    if (!file) return;
    setNaturalSize(null);
    setImageUrl((previous) => {
      if (previous) URL.revokeObjectURL(previous);
      return URL.createObjectURL(file);
    });
  }

  function handleImageLoad(event: SyntheticEvent<HTMLImageElement>) {
    const img = event.currentTarget;
    setNaturalSize({ width: img.naturalWidth, height: img.naturalHeight });
  }

  function startDrag(
    event: ReactMouseEvent<HTMLDivElement>,
    key: MappingRegionKey,
    mode: "move" | "resize",
  ) {
    event.preventDefault();
    event.stopPropagation();
    const box = boxes.find((candidate) => candidate.key === key);
    if (!box) return;
    setActiveDrag({ key, mode, startClientX: event.clientX, startClientY: event.clientY, startBox: box });
  }

  return (
    <div className="flex flex-1 flex-col overflow-auto bg-zinc-900/40 p-6">
      <div className="mb-4 flex items-center gap-3">
        <button
          type="button"
          onClick={() => fileInputRef.current?.click()}
          className="rounded-md border border-cyan-400/50 bg-cyan-400/10 px-4 py-2 text-xs font-semibold uppercase tracking-wider text-cyan-300 transition-colors hover:bg-cyan-400/20"
        >
          Carregar Screenshot de Referência
        </button>
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*"
          onChange={handleFileChange}
          className="hidden"
        />
        {naturalSize && (
          <p className="text-xs text-zinc-500">
            {naturalSize.width}×{naturalSize.height}px · exibido a {Math.round(scale * 100)}%
          </p>
        )}
      </div>

      {imageUrl ? (
        <div
          className="relative select-none border border-white/10"
          style={{
            width: naturalSize ? naturalSize.width * scale : undefined,
            height: naturalSize ? naturalSize.height * scale : undefined,
          }}
        >
          <img
            src={imageUrl}
            onLoad={handleImageLoad}
            alt="Screenshot de referência"
            className="block h-full w-full select-none"
            draggable={false}
          />

          {naturalSize &&
            boxes.map((box) => (
              <div
                key={box.key}
                onMouseDown={(event) => startDrag(event, box.key, "move")}
                className="absolute cursor-move border-2 border-cyan-400 bg-cyan-400/10"
                style={{
                  left: box.x * scale,
                  top: box.y * scale,
                  width: box.width * scale,
                  height: box.height * scale,
                }}
              >
                <span className="absolute -top-5 left-0 whitespace-nowrap rounded-sm bg-cyan-400 px-1.5 py-0.5 text-[10px] font-semibold text-zinc-950">
                  {box.label}
                </span>
                <div
                  onMouseDown={(event) => startDrag(event, box.key, "resize")}
                  className="absolute -bottom-1.5 -right-1.5 h-3 w-3 cursor-nwse-resize rounded-sm border border-zinc-950 bg-cyan-300"
                />
              </div>
            ))}
        </div>
      ) : (
        <div className="flex flex-1 items-center justify-center rounded-lg border border-dashed border-white/10 text-sm text-zinc-500">
          Nenhuma screenshot carregada. Envie uma referência (ex.: 1920×1080) para mapear as regiões.
        </div>
      )}
    </div>
  );
}
