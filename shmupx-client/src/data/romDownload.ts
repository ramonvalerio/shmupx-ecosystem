import { useSyncExternalStore } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Andamento do download dos ROMs (Rust `roms_auto::Progresso`, evento `roms://progresso`). */
export interface RomProgress {
  fase: "inicio" | "baixando" | "arquivo" | "fim";
  total: number;
  indice: number;
  jogo: string;
  rom: string;
  recebidos: number;
  tamanho: number | null;
  ok: boolean | null;
}

/** O que a modal de download mostra. */
export interface RomDownloadView {
  open: boolean;
  /** `reading`: lendo o `roms.json`; `downloading`: baixando; `done`: terminou. */
  stage: "reading" | "downloading" | "done";
  total: number;
  index: number;
  game: string;
  rom: string;
  received: number;
  size: number | null;
  done: string[];
  /** Ids dos jogos cujo ROM não deu para baixar (o motivo técnico fica no log). */
  failed: string[];
  /** Sem o catálogo (não entrou na conta): não dá para saber quais jogos baixar. */
  noCatalog: boolean;
  /** A pasta escolhida não tem `roms.json`: nada a baixar. */
  noList: boolean;
}

const CLOSED: RomDownloadView = { open: false, stage: "reading", total: 0, index: 0, game: "", rom: "", received: 0, size: null, done: [], failed: [], noCatalog: false, noList: false };

let view: RomDownloadView = CLOSED;
const listeners = new Set<() => void>();
let unlisten: Promise<UnlistenFn> | null = null;

function set(next: RomDownloadView) {
  view = next;
  listeners.forEach((listener) => listener());
}

function onProgress(p: RomProgress) {
  if (!view.open || view.stage === "done") return;
  if (p.fase === "inicio") set({ ...view, stage: "downloading", total: p.total, index: 0 });
  else if (p.fase === "baixando") set({ ...view, stage: "downloading", total: p.total, index: p.indice, game: p.jogo, rom: p.rom, received: p.recebidos, size: p.tamanho });
  else if (p.fase === "arquivo") set({ ...view, index: p.indice + 1, received: 0, size: null, done: p.ok ? [...view.done, p.rom] : view.done, failed: p.ok ? view.failed : [...view.failed, p.jogo] });
}

/** Abre a modal (lendo o `roms.json`) e passa a ouvir o andamento do download. */
export function openRomDownload() {
  set({ ...CLOSED, open: true });
  if (isTauri() && !unlisten) unlisten = listen<RomProgress>("roms://progresso", (event) => onProgress(event.payload));
}

/** Terminou: mostra o resumo (o resultado final do download manda sobre os avisos). */
export function finishRomDownload(result: { baixados: string[]; falhas: string[]; sem_lista?: boolean }, noCatalog = false) {
  if (!view.open) return;
  set({ ...view, stage: "done", noList: Boolean(result.sem_lista), done: result.baixados, failed: view.failed.length > 0 || result.falhas.length === 0 ? view.failed : result.falhas, noCatalog, total: Math.max(view.total, result.baixados.length + result.falhas.length) });
}

/** Fecha a modal e para de ouvir o andamento. */
export function closeRomDownload() {
  set(CLOSED);
  const pending = unlisten;
  unlisten = null;
  void pending?.then((stop) => stop());
}

export function useRomDownload(): RomDownloadView {
  return useSyncExternalStore((listener) => { listeners.add(listener); return () => { listeners.delete(listener); }; }, () => view);
}

/** 0 a 100: arquivos já terminados mais a parte baixada do atual. */
export function romDownloadPercent(v: Pick<RomDownloadView, "stage" | "total" | "index" | "received" | "size">): number {
  if (v.stage === "done") return 100;
  if (v.total <= 0) return 0;
  const current = v.size && v.size > 0 ? Math.min(1, v.received / v.size) : 0;
  return Math.round(Math.min(1, (v.index + current) / v.total) * 100);
}
