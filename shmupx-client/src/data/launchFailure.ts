import { invoke, isTauri } from "@tauri-apps/api/core";

/**
 * Falhas ao abrir o jogo: o detalhe técnico (SSH, rede, processo…) vai para o log do sistema e o
 * jogador vê só uma explicação simples, com o que ele pode fazer (`LaunchProblemDialog`).
 */
export function logFailure(origin: string, failure: unknown) {
  const detail = typeof failure === "string" ? failure : failure instanceof Error ? failure.message : JSON.stringify(failure);
  console.warn(`${origin}: ${detail}`);
  if (isTauri()) void invoke("registrar_falha", { origem: origin, detalhe: detail }).catch(() => undefined);
}

/** Por que o MiSTer não respondeu: este PC sem rede, em outra rede, ou o MiSTer calado. */
export type MisterReach = "sem_rede" | "outra_rede" | "sem_resposta";

/**
 * Conferência rápida (1,5 s) se o MiSTer responde na rede, antes de abrir ou parar um jogo nele:
 * evita esperar o tempo todo de uma conexão que não vai acontecer. `null` = respondeu.
 */
export async function checkMister(host: string): Promise<MisterReach | null> {
  if (!isTauri()) return null;
  try {
    return await invoke<MisterReach | null>("mister_alcance", { host });
  } catch {
    return null;
  }
}
