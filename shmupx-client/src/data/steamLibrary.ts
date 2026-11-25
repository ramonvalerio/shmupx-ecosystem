import { useCallback, useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";

/** Jogo instalado na Steam deste PC (Rust `steam::biblioteca`). */
export interface SteamGame {
  app_id: number;
  name: string;
  installed: boolean;
}

/** Biblioteca importada: os app_ids da Steam deste PC no momento da importação. */
export interface SteamLibrary {
  appIds: number[];
  /** Jogos encontrados na Steam (instalados ou não). */
  total: number;
  importedAt: number;
}

const KEY = "shmupx.client.steamLibrary.v1";
/** O jogador desconectou a Steam: o Client não importa sozinho até ele importar de novo. */
const OFF_KEY = "shmupx.client.steamLibrary.off.v1";
const EVENT = "shmupx:steam-library";

export function readSteamLibrary(): SteamLibrary | null {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) ?? "null") as SteamLibrary | null;
    if (!value || !Array.isArray(value.appIds)) return null;
    const appIds = value.appIds.filter((id) => Number.isInteger(id) && id > 0);
    return { appIds, total: Math.max(Number(value.total) || 0, appIds.length), importedAt: Number(value.importedAt) || 0 };
  } catch {
    return null;
  }
}

export function saveSteamLibrary(appIds: number[], total = appIds.length) {
  const unique = [...new Set(appIds)];
  const value: SteamLibrary = { appIds: unique, total: Math.max(total, unique.length), importedAt: Date.now() };
  try { localStorage.setItem(KEY, JSON.stringify(value)); localStorage.removeItem(OFF_KEY); } catch { /* fica só nesta sessão */ }
  window.dispatchEvent(new CustomEvent(EVENT, { detail: value }));
  return value;
}

/** O jogador desconectou a Steam (a importação automática está parada até ele importar de novo). */
export function steamDisconnected(): boolean {
  try { return localStorage.getItem(OFF_KEY) !== null; } catch { return false; }
}

/**
 * Desconecta a Steam: esquece a biblioteca importada (a coleção volta a "fora da biblioteca") e
 * para a importação automática a cada login, até o jogador importar de novo.
 */
export function clearSteamLibrary() {
  try { localStorage.removeItem(KEY); localStorage.setItem(OFF_KEY, "1"); } catch { /* nada a limpar */ }
  window.dispatchEvent(new CustomEvent(EVENT, { detail: null }));
}

/** Lê a Steam deste PC (só no app desktop). */
export async function scanSteam(): Promise<{ found: boolean; games: SteamGame[] }> {
  if (!isTauri()) return { found: false, games: [] };
  return invoke<{ found: boolean; games: SteamGame[] }>("steam_biblioteca");
}

/**
 * A cada login: lê a Steam deste PC e importa os jogos instalados, sem o jogador pedir (a coleção
 * já abre com eles em cor). `null`: sem Steam no PC; `"off"`: desconectada pelo jogador (não lê).
 */
export async function autoImportSteam(): Promise<number | null | "off"> {
  if (steamDisconnected()) return "off";
  const result = await scanSteam();
  if (!result.found) return null;
  return saveSteamLibrary(result.games.filter((g) => g.installed).map((g) => g.app_id), result.games.length).appIds.length;
}

/** Biblioteca importada, atualizada em todas as telas quando muda. */
export function useSteamLibrary() {
  const [library, setLibrary] = useState<SteamLibrary | null>(readSteamLibrary);
  useEffect(() => {
    const onChange = (event: Event) => setLibrary((event as CustomEvent<SteamLibrary | null>).detail);
    window.addEventListener(EVENT, onChange);
    return () => window.removeEventListener(EVENT, onChange);
  }, []);
  /** Lê de novo a Steam e guarda o resultado (botão "Atualizar biblioteca"). */
  const refresh = useCallback(async () => {
    const result = await scanSteam();
    if (result.found) saveSteamLibrary(result.games.filter((g) => g.installed).map((g) => g.app_id), result.games.length);
    return result;
  }, []);
  const owns = useCallback((appId: number) => Boolean(library?.appIds.includes(appId)), [library]);
  return { library, owns, refresh };
}
