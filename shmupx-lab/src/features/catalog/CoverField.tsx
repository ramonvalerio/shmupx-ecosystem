import { useCallback, useEffect, useRef, useState, type ChangeEvent, type DragEvent, type ReactNode } from "react";
import Cropper, { type Area } from "react-easy-crop";
import Button from "../../components/ui/Button";
import { IconClose } from "../../components/ui/Icons";
import { adjustFilter, COVER_FORMAT, DEFAULT_ADJUST, publishCover, renderCover, type CoverAdjust, type ImageFormat } from "../../lib/cover";

const MAX_SOURCE_BYTES = 25 * 1024 * 1024;

interface CoverFieldProps {
  gameId: string;
  title: string;
  value: string;
  onChange: (url: string) => void;
  /** Capa do card (padrão) ou banner de evento. */
  format?: ImageFormat;
}

/** Imagem do card do jogo no Client: escolher, recortar/ajustar, enviar. */
export default function CoverField({ gameId, title, value, onChange, format = COVER_FORMAT }: CoverFieldProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [source, setSource] = useState<string | null>(null);
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const openFile = useCallback((file: File | undefined) => {
    setError(null);
    if (!file) return;
    if (!file.type.startsWith("image/")) return setError("Escolha um arquivo de imagem (PNG, JPG, WebP, AVIF…).");
    if (file.size > MAX_SOURCE_BYTES) return setError("Imagem acima de 25 MB.");
    setSource(URL.createObjectURL(file));
  }, []);

  // Colar (Ctrl+V) uma imagem copiada enquanto o editor de jogo está aberto.
  useEffect(() => {
    if (source) return;
    const onPaste = (event: ClipboardEvent) => {
      const file = [...(event.clipboardData?.files ?? [])].find((f) => f.type.startsWith("image/"));
      if (file) { event.preventDefault(); openFile(file); }
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, [openFile, source]);

  function closeEditor() {
    if (source) URL.revokeObjectURL(source);
    setSource(null);
  }

  function onDrop(event: DragEvent) {
    event.preventDefault();
    setDragging(false);
    openFile(event.dataTransfer.files[0]);
  }

  return (
    <div>
      <p className="mb-1.5 text-xs font-medium text-zinc-400">{format.label}</p>
      <div
        onDragOver={(e) => { e.preventDefault(); setDragging(true); }}
        onDragLeave={() => setDragging(false)}
        onDrop={onDrop}
        className={`group relative ${format.aspectClass} w-full overflow-hidden rounded-lg ring-1 transition-colors ${dragging ? "bg-cyan-400/10 ring-cyan-400/60" : "bg-zinc-900 ring-white/10"}`}
      >
        {value ? (
          <img src={value} alt={`Capa de ${title || "jogo"}`} className="h-full w-full object-cover" />
        ) : (
          <button type="button" onClick={() => inputRef.current?.click()} className="flex h-full w-full flex-col items-center justify-center gap-1.5 text-center text-xs text-zinc-500 hover:text-zinc-300 focus-visible:outline-none">
            <span className="text-sm font-medium text-zinc-300">Arraste, cole (Ctrl+V) ou clique</span>
            <span>Recorte 16:9 · sai em WebP 1280×720</span>
          </button>
        )}
        {value && (
          <div className="absolute inset-x-0 bottom-0 flex justify-end gap-2 bg-gradient-to-t from-black/80 to-transparent p-2 opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100">
            <Button size="sm" onClick={() => inputRef.current?.click()}>Trocar</Button>
            <Button size="sm" variant="danger" onClick={() => onChange("")}>Remover</Button>
          </div>
        )}
      </div>
      <input ref={inputRef} type="file" accept="image/*" hidden onChange={(e: ChangeEvent<HTMLInputElement>) => { openFile(e.target.files?.[0]); e.target.value = ""; }} />
      <p className="mt-1.5 text-xs text-zinc-500">{error ? <span className="text-red-300">{error}</span> : "Sem imagem, o Client mostra o título no card. A troca vale ao salvar o jogo."}</p>
      {source && (
        <CoverEditor
          source={source}
          gameId={gameId}
          format={format}
          title={title}
          onCancel={closeEditor}
          onDone={(url) => { onChange(url); closeEditor(); }}
        />
      )}
    </div>
  );
}

function Slider({ label, value, min, max, step = 1, suffix = "", onChange }: { label: string; value: number; min: number; max: number; step?: number; suffix?: string; onChange: (v: number) => void }) {
  return (
    <label className="block">
      <span className="flex justify-between text-xs text-zinc-400"><span>{label}</span><span className="font-mono text-zinc-500">{Number.isInteger(step) ? value : value.toFixed(2)}{suffix}</span></span>
      <input type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} className="mt-1 w-full accent-cyan-400" />
    </label>
  );
}

function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="space-y-3">
      <h4 className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">{title}</h4>
      {children}
    </section>
  );
}

function CoverEditor({ source, gameId, title, format, onCancel, onDone }: { source: string; gameId: string; title: string; format: ImageFormat; onCancel: () => void; onDone: (url: string) => void }) {
  const [crop, setCrop] = useState({ x: 0, y: 0 });
  const [zoom, setZoom] = useState(1);
  const [adjust, setAdjust] = useState<CoverAdjust>(DEFAULT_ADJUST);
  const [area, setArea] = useState<Area | null>(null);
  const [preview, setPreview] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const set = (changes: Partial<CoverAdjust>) => setAdjust((a) => ({ ...a, ...changes }));

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => { if (event.key === "Escape" && !busy) { event.stopPropagation(); onCancel(); } };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [busy, onCancel]);

  // Prévia do card com o resultado real (mesma rotina da exportação), com um pequeno atraso.
  useEffect(() => {
    if (!area) return;
    let url: string | null = null;
    const timer = setTimeout(async () => {
      try {
        const bytes = await renderCover(source, area, adjust, format);
        url = URL.createObjectURL(new Blob([bytes], { type: "image/webp" }));
        setPreview((old) => { if (old) URL.revokeObjectURL(old); return url; });
      } catch { /* a prévia é só visual */ }
    }, 250);
    return () => clearTimeout(timer);
  }, [source, area, adjust]);

  function reset() {
    setCrop({ x: 0, y: 0 });
    setZoom(1);
    setAdjust(DEFAULT_ADJUST);
  }

  async function apply() {
    if (!area) return;
    if (!gameId.trim()) return setError("Defina o ID antes de enviar a imagem.");
    setBusy(true);
    setError(null);
    try {
      const bytes = await renderCover(source, area, adjust, format);
      onDone(await publishCover(gameId.trim(), bytes, format.folder));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  }

  return (
    <div role="dialog" aria-modal="true" aria-label="Ajustar imagem do card" className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-6 backdrop-blur-sm">
      <div className="flex max-h-full w-full max-w-[1100px] flex-col overflow-hidden rounded-xl border border-white/10 bg-[#0e0e11] shadow-2xl">
        <header className="flex items-center justify-between border-b border-white/5 px-5 py-3">
          <div>
            <p className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">{format.label}</p>
            <h3 className="text-sm font-semibold text-zinc-100">{title || "Novo jogo"} · recorte 16:9</h3>
          </div>
          <button type="button" onClick={onCancel} disabled={busy} aria-label="Fechar" className="rounded-md p-1.5 text-zinc-500 hover:bg-white/5 hover:text-zinc-200"><IconClose /></button>
        </header>

        <div className="grid min-h-0 flex-1 grid-cols-1 overflow-y-auto lg:grid-cols-[minmax(0,1fr)_300px]">
          <div className="relative min-h-[360px] bg-[repeating-conic-gradient(#18181b_0_25%,#111113_0_50%)] bg-[length:20px_20px] lg:min-h-[520px]">
            <Cropper
              image={source}
              crop={crop}
              zoom={zoom}
              rotation={adjust.rotation}
              aspect={format.aspect}
              minZoom={0.5}
              maxZoom={5}
              zoomSpeed={0.15}
              restrictPosition={zoom >= 1}
              showGrid
              objectFit="contain"
              onCropChange={setCrop}
              onZoomChange={setZoom}
              onRotationChange={(rotation) => set({ rotation })}
              onCropComplete={(_, pixels) => setArea(pixels)}
              style={{ mediaStyle: { filter: adjustFilter(adjust) }, cropAreaStyle: { border: "2px solid #57FCFF", color: "rgba(0,0,0,.6)" } }}
              transform={`translate(${crop.x}px, ${crop.y}px) rotate(${adjust.rotation}deg) scale(${zoom}) scaleX(${adjust.flipX ? -1 : 1})`}
            />
          </div>

          <aside className="space-y-6 border-l border-white/5 p-5">
            <Group title="Enquadramento">
              <Slider label="Zoom" value={zoom} min={0.5} max={5} step={0.01} suffix="×" onChange={setZoom} />
              <Slider label="Rotação" value={adjust.rotation} min={-180} max={180} suffix="°" onChange={(rotation) => set({ rotation })} />
              <div className="flex flex-wrap gap-2">
                <Button size="sm" onClick={() => set({ rotation: ((adjust.rotation - 90 + 540) % 360) - 180 })}>⟲ 90°</Button>
                <Button size="sm" onClick={() => set({ rotation: ((adjust.rotation + 90 + 540) % 360) - 180 })}>⟳ 90°</Button>
                <Button size="sm" aria-pressed={adjust.flipX} onClick={() => set({ flipX: !adjust.flipX })}>⇋ Espelhar</Button>
              </div>
              <p className="text-xs text-zinc-500">Arraste para posicionar; role o mouse ou faça pinça para o zoom.</p>
            </Group>

            <Group title="Cor">
              <Slider label="Brilho" value={adjust.brightness} min={50} max={150} suffix="%" onChange={(brightness) => set({ brightness })} />
              <Slider label="Contraste" value={adjust.contrast} min={50} max={150} suffix="%" onChange={(contrast) => set({ contrast })} />
              <Slider label="Saturação" value={adjust.saturation} min={0} max={200} suffix="%" onChange={(saturation) => set({ saturation })} />
            </Group>

            <Group title="Prévia no Client">
              <div className="overflow-hidden rounded-md bg-[#262626] ring-1 ring-white/10">
                <div className={`${format.aspectClass} bg-[#1f1f1f]`}>{preview && <img src={preview} alt="" className="h-full w-full object-cover" />}</div>
                <p className="truncate px-3 py-2 text-xs font-semibold text-zinc-100">{title || "Novo jogo"}</p>
              </div>
              <p className="text-xs text-zinc-500">Sai em WebP 1280×720.</p>
            </Group>
          </aside>
        </div>

        <footer className="flex items-center gap-3 border-t border-white/5 px-5 py-3">
          <p role={error ? "alert" : undefined} className="min-w-0 flex-1 text-xs text-red-300">{error}</p>
          <Button variant="ghost" onClick={reset} disabled={busy}>Redefinir</Button>
          <Button variant="info" onClick={onCancel} disabled={busy}>Cancelar</Button>
          <Button variant="success" onClick={() => void apply()} loading={busy} disabled={!area}>Aplicar e enviar</Button>
        </footer>
      </div>
    </div>
  );
}
