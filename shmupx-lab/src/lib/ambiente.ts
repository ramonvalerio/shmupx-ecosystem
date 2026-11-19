/**
 * Ambiente do Lab, escolhido ao abrir o app (tela inicial) antes de o Firebase iniciar:
 * - `dev`: emuladores do Firebase em Docker (shmupx-infra/firebase, projeto demo-shmupx) e
 *   backend local. Nada toca produção.
 * - `prd`: projeto Firebase real (shmupx-project-id) e o backend publicado no Cloud Run.
 *
 * `VITE_SHMUPX_ENV` (`dev` ou `prd`) fixa o ambiente na build e pula a tela de escolha.
 */
export type Ambiente = "dev" | "prd";

const forcado = import.meta.env.VITE_SHMUPX_ENV;

/** Ambiente fixado por `VITE_SHMUPX_ENV`; `null` quando o Lab deve perguntar. */
export const AMBIENTE_FORCADO: Ambiente | null = forcado ? (forcado === "prd" ? "prd" : "dev") : null;

/** Endereço do shmupx-backend em cada ambiente. As variáveis do Vite só sobrescrevem o padrão. */
const BACKEND_URLS: Record<Ambiente, string> = {
  dev: import.meta.env.VITE_BACKEND_URL_DEV ?? import.meta.env.VITE_BACKEND_URL ?? "http://localhost:3000",
  prd: import.meta.env.VITE_BACKEND_URL_PRD ?? "https://shmupx-backend-230979440400.us-central1.run.app",
};

export function backendUrlDe(ambiente: Ambiente): string {
  return BACKEND_URLS[ambiente].replace(/\/+$/, "");
}

const CHAVE_ULTIMO = "shmupx-lab:ultimo-ambiente";

/** Última escolha feita na tela inicial (só para pré-selecionar; o Lab sempre pergunta). */
export function ultimoAmbiente(): Ambiente | null {
  try {
    const valor = localStorage.getItem(CHAVE_ULTIMO);
    return valor === "dev" || valor === "prd" ? valor : null;
  } catch {
    return null;
  }
}

export function lembrarAmbiente(ambiente: Ambiente) {
  try {
    localStorage.setItem(CHAVE_ULTIMO, ambiente);
  } catch {
    // Sem localStorage a escolha só não fica pré-selecionada na próxima vez.
  }
}