import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import { useDialogFocus } from "./useDialogFocus";

const VIEW = 280;
const OUTPUT = 160;
/** O backend aceita até ~80 mil caracteres de data URL. */
const MAX_DATA_URL = 75_000;

interface Placement { zoom: number; x: number; y: number; rotation: number }

/** Desenha a imagem no quadrado: centro + deslocamento, giro e escala (cobre o quadrado em zoom 1). */
function draw(canvas: HTMLCanvasElement, image: HTMLImageElement, placement: Placement, size: number) {
  const context = canvas.getContext("2d");
  if (!context) return;
  canvas.width = size;
  canvas.height = size;
  const factor = size / VIEW;
  const turned = placement.rotation % 180 !== 0;
  const width = turned ? image.naturalHeight : image.naturalWidth;
  const height = turned ? image.naturalWidth : image.naturalHeight;
  const cover = Math.max(VIEW / width, VIEW / height) * placement.zoom * factor;
  context.fillStyle = "#111";
  context.fillRect(0, 0, size, size);
  context.save();
  context.translate(size / 2 + placement.x * factor, size / 2 + placement.y * factor);
  context.rotate((placement.rotation * Math.PI) / 180);
  context.scale(cover, cover);
  context.imageSmoothingQuality = "high";
  context.drawImage(image, -image.naturalWidth / 2, -image.naturalHeight / 2);
  context.restore();
}

/** Recorta para 160x160: WebP (ou JPEG, onde o WebView não gera WebP), dentro do limite. */
function exportAvatar(image: HTMLImageElement, placement: Placement): string {
  const canvas = document.createElement("canvas");
  draw(canvas, image, placement, OUTPUT);
  for (const quality of [0.9, 0.8, 0.7, 0.55]) {
    let url = canvas.toDataURL("image/webp", quality);
    if (!url.startsWith("data:image/webp")) url = canvas.toDataURL("image/jpeg", quality);
    if (url.length <= MAX_DATA_URL) return url;
  }
  throw new Error("A imagem ficou grande demais. Tente outra imagem.");
}

/**
 * Editor do avatar: escolher (ou arrastar) a imagem, mover arrastando, ampliar (roda do mouse ou
 * controle), girar e recortar no círculo. Salva 160x160 no perfil público.
 */
export default function AvatarEditor({ current, onClose, onSaved }: { current: string | null; onClose: () => void; onSaved: () => void }) {
  const { t } = useI18n();
  const dialogRef = useDialogFocus(true, onClose);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const [image, setImage] = useState<HTMLImageElement | null>(null);
  const [placement, setPlacement] = useState<Placement>({ zoom: 1, x: 0, y: 0, rotation: 0 });
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const drag = useRef<{ x: number; y: number; start: Placement } | null>(null);

  function load(file: File | undefined) {
    if (!file) return;
    if (!file.type.startsWith("image/")) { setError(t("Escolha um arquivo de imagem (PNG, JPG, WebP, GIF).")); return; }
    if (file.size > 15 * 1024 * 1024) { setError(t("Imagem grande demais (máximo 15 MB).")); return; }
    const url = URL.createObjectURL(file);
    const next = new Image();
    next.onload = () => { setImage(next); setPlacement({ zoom: 1, x: 0, y: 0, rotation: 0 }); setError(null); };
    next.onerror = () => setError(t("Não foi possível abrir essa imagem."));
    next.src = url;
  }

  // Imagem atual do perfil como ponto de partida.
  useEffect(() => {
    if (!current) return;
    const start = new Image();
    start.onload = () => setImage(start);
    start.src = current;
  }, [current]);

  useEffect(() => {
    if (canvasRef.current && image) draw(canvasRef.current, image, placement, VIEW);
  }, [image, placement]);

  function onPointerDown(event: ReactPointerEvent<HTMLCanvasElement>) {
    if (!image) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { x: event.clientX, y: event.clientY, start: placement };
  }
  function onPointerMove(event: ReactPointerEvent<HTMLCanvasElement>) {
    const origin = drag.current;
    if (!origin) return;
    setPlacement({ ...origin.start, x: origin.start.x + event.clientX - origin.x, y: origin.start.y + event.clientY - origin.y });
  }
  const zoomBy = (delta: number) => setPlacement((p) => ({ ...p, zoom: Math.min(5, Math.max(1, +(p.zoom + delta).toFixed(2))) }));

  async function save() {
    if (!image) return;
    setSaving(true);
    setError(null);
    try {
      await invoke("perfil_avatar", { avatar: exportAvatar(image, placement) });
      onSaved();
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setSaving(false);
    }
  }

  async function remove() {
    setSaving(true);
    try { await invoke("perfil_avatar", { avatar: null }); onSaved(); }
    catch (failure) { setError(failure instanceof Error ? failure.message : String(failure)); }
    finally { setSaving(false); }
  }

  return (
    <div className="controller-dialog-backdrop avatar-editor-backdrop">
      <div ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby="avatar-editor-title" tabIndex={-1} className="controller-dialog avatar-editor">
        <header>
          <div>
            <h2 id="avatar-editor-title">{t("Avatar")}</h2>
            <p>{t("Arraste para mover, use a roda do mouse ou o controle para aproximar. O círculo é o que aparece no ShmupX.")}</p>
          </div>
        </header>
        <div className="controller-dialog-body avatar-editor-body">
          <div
            className={`avatar-stage ${image ? "" : "is-empty"}`}
            onDragOver={(event) => event.preventDefault()}
            onDrop={(event) => { event.preventDefault(); load(event.dataTransfer.files[0]); }}
          >
            {image ? (
              <canvas
                ref={canvasRef}
                width={VIEW}
                height={VIEW}
                onPointerDown={onPointerDown}
                onPointerMove={onPointerMove}
                onPointerUp={() => { drag.current = null; }}
                onWheel={(event) => zoomBy(event.deltaY < 0 ? 0.1 : -0.1)}
                aria-label={t("Pré-visualização do avatar")}
              />
            ) : (
              <button type="button" className="avatar-pick" onClick={() => fileRef.current?.click()}>{t("Escolher imagem ou arrastar aqui")}</button>
            )}
            <span className="avatar-mask" aria-hidden="true" />
          </div>
          <input ref={fileRef} type="file" accept="image/*" hidden onChange={(event) => { load(event.target.files?.[0]); event.target.value = ""; }} />
          {image && (
            <div className="avatar-controls">
              <button type="button" className="secondary-action" onClick={() => zoomBy(-0.2)} aria-label={t("Afastar")}>−</button>
              <input type="range" min={1} max={5} step={0.01} value={placement.zoom} onChange={(event) => setPlacement((p) => ({ ...p, zoom: Number(event.target.value) }))} aria-label={t("Zoom")} />
              <button type="button" className="secondary-action" onClick={() => zoomBy(0.2)} aria-label={t("Aproximar")}>+</button>
              <button type="button" className="secondary-action" onClick={() => setPlacement((p) => ({ ...p, rotation: (p.rotation + 90) % 360 }))} title={t("Girar")}>⟳</button>
              <button type="button" className="secondary-action" onClick={() => setPlacement((p) => ({ ...p, zoom: 1, x: 0, y: 0 }))}>{t("Centralizar")}</button>
              <button type="button" className="text-action" onClick={() => fileRef.current?.click()}>{t("Trocar imagem")}</button>
            </div>
          )}
          {error && <p className="mode-error-inline" role="alert">{t(error)}</p>}
        </div>
        <footer>
          {current ? <button type="button" className="text-action" disabled={saving} onClick={() => void remove()}>{t("Remover avatar")}</button> : <span />}
          <div className="avatar-editor-actions">
            <button type="button" className="secondary-action" onClick={onClose} disabled={saving}>{t("Cancelar")}</button>
            <button type="button" className="primary-action" onClick={() => void save()} disabled={saving || !image}>{t(saving ? "Salvando…" : "Salvar avatar")}</button>
          </div>
        </footer>
      </div>
    </div>
  );
}
