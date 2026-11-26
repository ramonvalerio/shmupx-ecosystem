import { useEffect, useSyncExternalStore } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";

/**
 * "Gravar gameplay": a mesma opção na sala (ao lado da Live no YouTube) e em Configurações ›
 * Armazenamento. Fica no Rust (`capture::destino`, desmarcada por padrão); aqui só o espelho para as
 * telas mudarem juntas. Com a live ligada a partida não grava (a live fica no YouTube).
 */
let value: boolean | null = null;
let loading: Promise<void> | null = null;
const listeners = new Set<() => void>();

function notify(next: boolean) {
  value = next;
  listeners.forEach((listener) => listener());
}

function load() {
  if (!isTauri()) return;
  loading ??= invoke<{ gravar: boolean }>("gravacoes_destino")
    .then((destino) => notify(destino.gravar))
    .catch(() => undefined)
    .finally(() => { loading = null; });
}

/** Liga ou desliga "Gravar gameplay"; devolve o destino atualizado (o que o Rust gravou). */
export async function setRecordGameplay<T extends { gravar: boolean }>(on: boolean): Promise<T> {
  const destino = await invoke<T>("gravacoes_definir_gravar", { gravar: on });
  notify(destino.gravar);
  return destino;
}

/** `null` enquanto carrega (ou fora do app). */
export function useRecordGameplay(): boolean | null {
  const current = useSyncExternalStore((listener) => { listeners.add(listener); return () => listeners.delete(listener); }, () => value);
  useEffect(() => { if (value === null) load(); }, []);
  return current;
}
