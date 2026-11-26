import { invoke } from "@tauri-apps/api/core";
import type { Area } from "react-easy-crop";

/** Card do Client: 16:9, exportado em 1280×720 (nítido também no Big Picture). */
export const COVER_ASPECT = 16 / 9;
export const COVER_WIDTH = 1280;
export const COVER_HEIGHT = 720;

/** Formato da imagem enviada pelo Lab: a capa do card do jogo ou o banner de um evento. */
export interface ImageFormat {
  label: string;
  aspect: number;
  width: number;
  height: number;
  /** Pasta no site: `covers` (`catalog/covers/`) ou `banners` (`events/banners/`). */
  folder: "covers" | "banners";
  /** Classe de proporção da prévia (Tailwind). */
  aspectClass: string;
}
export const COVER_FORMAT: ImageFormat = { label: "Imagem do card", aspect: COVER_ASPECT, width: COVER_WIDTH, height: COVER_HEIGHT, folder: "covers", aspectClass: "aspect-video" };
/** Banner do carrossel de eventos na sala do Client: faixa larga 3:1, exportada em 1500×500. */
export const BANNER_FORMAT: ImageFormat = { label: "Banner do evento", aspect: 3, width: 1500, height: 500, folder: "banners", aspectClass: "aspect-[3/1]" };
const QUALITY = 0.86;

export interface CoverAdjust {
  rotation: number;
  flipX: boolean;
  brightness: number;
  contrast: number;
  saturation: number;
}

export const DEFAULT_ADJUST: CoverAdjust = { rotation: 0, flipX: false, brightness: 100, contrast: 100, saturation: 100 };

/** Filtro CSS/canvas equivalente aos ajustes (o mesmo na prévia e na exportação). */
export const adjustFilter = (a: CoverAdjust) => `brightness(${a.brightness}%) contrast(${a.contrast}%) saturate(${a.saturation}%)`;

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("Não foi possível abrir a imagem."));
    image.src = src;
  });
}

/**
 * Recorta a área escolhida (em pixels da imagem já girada, como o react-easy-crop entrega),
 * aplica espelho e ajustes e redimensiona para 1280×720 em WebP.
 */
export async function renderCover(src: string, area: Area, adjust: CoverAdjust, format: ImageFormat = COVER_FORMAT): Promise<Uint8Array<ArrayBuffer>> {
  const image = await loadImage(src);
  const radians = (adjust.rotation * Math.PI) / 180;
  const sin = Math.abs(Math.sin(radians));
  const cos = Math.abs(Math.cos(radians));
  const boundW = image.width * cos + image.height * sin;
  const boundH = image.width * sin + image.height * cos;

  // 1) imagem inteira girada/espelhada/ajustada numa tela do tamanho do contorno girado
  const rotated = document.createElement("canvas");
  rotated.width = Math.round(boundW);
  rotated.height = Math.round(boundH);
  const rctx = rotated.getContext("2d");
  if (!rctx) throw new Error("Canvas indisponível.");
  rctx.filter = adjustFilter(adjust);
  rctx.translate(boundW / 2, boundH / 2);
  rctx.rotate(radians);
  rctx.scale(adjust.flipX ? -1 : 1, 1);
  rctx.drawImage(image, -image.width / 2, -image.height / 2);

  // 2) recorte redimensionado direto para o tamanho final, com suavização alta
  const out = document.createElement("canvas");
  out.width = format.width;
  out.height = format.height;
  const octx = out.getContext("2d");
  if (!octx) throw new Error("Canvas indisponível.");
  octx.imageSmoothingEnabled = true;
  octx.imageSmoothingQuality = "high";
  octx.fillStyle = "#1f1f1f";
  octx.fillRect(0, 0, format.width, format.height);
  octx.drawImage(rotated, area.x, area.y, area.width, area.height, 0, 0, format.width, format.height);

  const blob = await new Promise<Blob | null>((resolve) => out.toBlob(resolve, "image/webp", QUALITY));
  if (!blob || blob.type !== "image/webp") throw new Error("Este sistema não conseguiu gerar WebP.");
  return new Uint8Array(await blob.arrayBuffer());
}

/** Envia a imagem ao S3 (credenciais do OpenBao, pelo Lab) e devolve a URL pública. `id`: o do jogo ou do evento. */
export const publishCover = (id: string, bytes: Uint8Array, folder: ImageFormat["folder"] = "covers") =>
  invoke<string>("publicar_capa", { gameId: id, bytes: Array.from(bytes), pasta: folder });
