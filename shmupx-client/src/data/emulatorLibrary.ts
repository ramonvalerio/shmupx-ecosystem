import { useCallback, useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { GameRoom } from "../types/room";
import { romsFor } from "./emulatorRoms";

export { romsFor };

/**
 * Biblioteca de emulador (Fightcade/FBNeo): os ROMs `.zip` que o jogador tem. "Ter o jogo" no
 * emulador = ter o ROM (como no Client antigo); a ligação ROM ↔ jogo fica em `emulatorRoms.ts`.
 */

export interface EmulatorLibrary {
  /** Pasta do Fightcade em que os ROMs foram lidos. */
  root: string;
  roms: string[];
  importedAt: number;
}

const KEY = "shmupx.client.emulatorLibrary.v1";
const EVENT = "shmupx:emulator-library";

export function readEmulatorLibrary(): EmulatorLibrary | null {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) ?? "null") as EmulatorLibrary | null;
    return value && Array.isArray(value.roms) ? { root: String(value.root ?? ""), roms: value.roms.filter((r) => typeof r === "string"), importedAt: Number(value.importedAt) || 0 } : null;
  } catch {
    return null;
  }
}

export function publish(value: EmulatorLibrary | null) {
  try {
    if (value) localStorage.setItem(KEY, JSON.stringify(value));
    else localStorage.removeItem(KEY);
  } catch { /* fica só nesta sessão */ }
  window.dispatchEvent(new CustomEvent(EVENT, { detail: value }));
}

/** Resultado da leitura do Fightcade (Rust `BibliotecaEmulador`). */
export interface EmulatorScan {
  found: boolean;
  /** Raiz do Fightcade (ou, sem o emulador, a pasta de ROMs escolhida). */
  root?: string | null;
  roms: string[];
  /** A pasta usada é a que o jogador escolheu (não a da busca automática). */
  manual?: boolean;
  /** Pasta escolhida pelo jogador e guardada no app, se houver. */
  pasta_escolhida?: string | null;
  /** A pasta escolhida não tem mais o Fightcade (disco removido?): usando a busca automática. */
  indisponivel?: boolean;
  /** Achou o emulador (`fcadefbneo.exe`): dá para abrir jogos, não só ler os ROMs. */
  emulador?: boolean;
}

/** Lê o Fightcade deste PC (só no app desktop). A pasta escolhida pelo jogador, guardada no app, vem antes de `folder`. */
export async function scanEmulator(folder?: string): Promise<EmulatorScan> {
  if (!isTauri()) return { found: false, roms: [] };
  return invoke("emulador_biblioteca", { pasta: folder ?? null });
}

/**
 * Abre a janela do sistema para o jogador indicar a pasta do Fightcade (a raiz, `emulator`, `fbneo`
 * ou a de ROMs), guarda a escolha no app e já lê os ROMs dela. `null` = o jogador cancelou. Se a
 * pasta não parece do Fightcade, rejeita com o motivo (em português). Se a biblioteca já estava
 * importada, ela é atualizada com a pasta nova.
 */
export async function chooseEmulatorFolder(title: string): Promise<EmulatorScan | null> {
  if (!isTauri()) return null;
  const current = readEmulatorLibrary()?.root || undefined;
  const folder = await open({ directory: true, multiple: false, title, defaultPath: current });
  if (typeof folder !== "string") return null;
  const result = await invoke<EmulatorScan>("emulador_definir_pasta", { pasta: folder });
  if (result.found && readEmulatorLibrary()) publish({ root: result.root ?? "", roms: result.roms, importedAt: Date.now() });
  return result;
}

/** Esquece a pasta escolhida e volta à busca automática do Fightcade (atualiza a biblioteca importada). */
export async function resetEmulatorFolder(): Promise<EmulatorScan> {
  if (!isTauri()) return { found: false, roms: [] };
  const result = await invoke<EmulatorScan>("emulador_definir_pasta", { pasta: null });
  if (readEmulatorLibrary()) publish(result.found ? { root: result.root ?? "", roms: result.roms, importedAt: Date.now() } : null);
  return result;
}

/** Pasta a mostrar ao jogador: a escolhida (se está em uso) ou a encontrada. */
export function emulatorFolderLabel(scan: Pick<EmulatorScan, "root" | "manual" | "pasta_escolhida">): string {
  return (scan.manual && scan.pasta_escolhida) || scan.root || "";
}

/** Biblioteca do emulador importada, atualizada em todas as telas quando muda. */
export function useEmulatorLibrary() {
  const [library, setLibrary] = useState<EmulatorLibrary | null>(readEmulatorLibrary);
  useEffect(() => {
    const onChange = (event: Event) => setLibrary((event as CustomEvent<EmulatorLibrary | null>).detail);
    window.addEventListener(EVENT, onChange);
    return () => window.removeEventListener(EVENT, onChange);
  }, []);
  /** Lê de novo o Fightcade (na pasta já conhecida) e guarda o resultado. */
  const refresh = useCallback(async () => {
    const result = await scanEmulator(readEmulatorLibrary()?.root || undefined);
    if (result.found) publish({ root: result.root ?? "", roms: result.roms, importedAt: Date.now() });
    return result;
  }, []);
  /** Escolhe outra pasta do Fightcade (janela do sistema); `null` = cancelou. */
  const chooseFolder = useCallback((title: string) => chooseEmulatorFolder(title), []);
  const disconnect = useCallback(() => publish(null), []);
  const owns = useCallback((room: GameRoom) => Boolean(library && romsFor(room).some((rom) => library.roms.includes(rom))), [library]);
  return { library, owns, refresh, chooseFolder, disconnect };
}
