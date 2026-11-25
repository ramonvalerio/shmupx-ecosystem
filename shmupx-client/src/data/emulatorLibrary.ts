import { useCallback, useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { GameRoom } from "../types/room";
import { romsFor } from "./emulatorRoms";

export { romsFor };

/**
 * Biblioteca do ShmupX Emulator: os ROMs `.zip` que o jogador tem. "Ter o jogo" no
 * emulador = ter o ROM (como no Client antigo); a ligação ROM ↔ jogo fica em `emulatorRoms.ts`.
 */

export interface EmulatorLibrary {
  /** Pasta em que os ROMs foram lidos. */
  root: string;
  roms: string[];
  importedAt: number;
}

const KEY = "shmupx.client.emulatorLibrary.v1";
/** O jogador desconectou o emulador: o Client não importa sozinho até ele importar de novo. */
const OFF_KEY = "shmupx.client.emulatorLibrary.off.v1";
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
    if (value) { localStorage.setItem(KEY, JSON.stringify(value)); localStorage.removeItem(OFF_KEY); }
    else localStorage.removeItem(KEY);
  } catch { /* fica só nesta sessão */ }
  window.dispatchEvent(new CustomEvent(EVENT, { detail: value }));
}

function emulatorOff() {
  try { return localStorage.getItem(OFF_KEY) !== null; } catch { return false; }
}

/** Desconecta o emulador (botão Desconectar): esquece os ROMs e para a importação automática. */
function disconnectEmulator() {
  try { localStorage.setItem(OFF_KEY, "1"); } catch { /* vale só nesta sessão */ }
  publish(null);
}

/** Resultado da leitura dos ROMs (Rust `BibliotecaEmulador`). */
export interface EmulatorScan {
  found: boolean;
  /** Pasta dos ROMs. */
  root?: string | null;
  roms: string[];
  /** A pasta usada é a que o jogador escolheu (não a padrão, `roms` do Client). */
  manual?: boolean;
  /** Pasta escolhida pelo jogador e guardada no app, se houver. */
  pasta_escolhida?: string | null;
  /** A pasta escolhida não tem mais ROMs (disco removido?): usando a padrão. */
  indisponivel?: boolean;
  /** Achou o emulador (`fcadefbneo.exe`): dá para abrir jogos, não só ler os ROMs. */
  emulador?: boolean;
}

/** Lê os ROMs deste PC (só no app desktop). A pasta escolhida pelo jogador, guardada no app, vem antes de `folder`. */
export async function scanEmulator(folder?: string): Promise<EmulatorScan> {
  if (!isTauri()) return { found: false, roms: [] };
  return invoke("emulador_biblioteca", { pasta: folder ?? null });
}

/** Resultado do download automático dos ROMs (Rust `ResultadoDownload`). */
export interface RomDownload {
  baixados: string[];
  falhas: string[];
  em_andamento: boolean;
}

/**
 * Baixa para a pasta `roms` do Client os ROMs da lista do Client (`src-tauri/roms.json`) que faltam
 * neste PC. Se baixou algum, lê os ROMs de novo e atualiza a biblioteca: os jogos ganham cor na coleção.
 */
export async function downloadMissingRoms(): Promise<RomDownload> {
  if (!isTauri()) return { baixados: [], falhas: [], em_andamento: false };
  const result = await invoke<RomDownload>("emulador_baixar_roms");
  if (result.falhas.length > 0) console.warn("ROMs: não deu para baixar:", result.falhas.join(", "));
  if (result.baixados.length > 0 && !emulatorOff()) {
    const scan = await scanEmulator(readEmulatorLibrary()?.root || undefined);
    if (scan.found) publish({ root: scan.root ?? "", roms: scan.roms, importedAt: Date.now() });
  }
  return result;
}

/**
 * Ao abrir o Client: importa os ROMs deste PC (a pasta escolhida, a `roms` do Client e as do
 * Fightcade), sem o jogador pedir, e depois baixa os ROMs da lista que faltam (o download lê de novo
 * a pasta se baixou algum). Ler primeiro deixa a coleção em cor sem esperar o download. Com o
 * emulador desconectado, só baixa. Pasta sem nenhum ROM não conecta a biblioteca (só atualiza a que
 * já existia).
 */
export async function autoImportEmulator() {
  if (!isTauri()) return;
  if (!emulatorOff()) {
    const current = readEmulatorLibrary();
    const result = await scanEmulator(current?.root || undefined);
    if (result.found && (result.roms.length > 0 || current)) publish({ root: result.root ?? "", roms: result.roms, importedAt: Date.now() });
  }
  await downloadMissingRoms().catch((error) => console.warn("ROMs: falha no download automático:", error));
}

/**
 * Abre a janela do sistema para o jogador indicar a pasta de ROMs, guarda a escolha no app e já lê
 * os ROMs dela. `null` = o jogador cancelou. Se a pasta não tem ROMs, rejeita com o motivo (em
 * português). Se a biblioteca já estava
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

/** Esquece a pasta escolhida e volta à pasta padrão dos ROMs (atualiza a biblioteca importada). */
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
  /** Baixa os ROMs que faltam, lê de novo os ROMs (na pasta já conhecida) e guarda o resultado. */
  const refresh = useCallback(async () => {
    await downloadMissingRoms().catch((error) => console.warn("ROMs: falha no download automático:", error));
    const result = await scanEmulator(readEmulatorLibrary()?.root || undefined);
    if (result.found) publish({ root: result.root ?? "", roms: result.roms, importedAt: Date.now() });
    return result;
  }, []);
  /** Escolhe outra pasta de ROMs (janela do sistema); `null` = cancelou. */
  const chooseFolder = useCallback((title: string) => chooseEmulatorFolder(title), []);
  const disconnect = useCallback(() => disconnectEmulator(), []);
  const owns = useCallback((room: GameRoom) => Boolean(library && romsFor(room).some((rom) => library.roms.includes(rom))), [library]);
  return { library, owns, refresh, chooseFolder, disconnect };
}
