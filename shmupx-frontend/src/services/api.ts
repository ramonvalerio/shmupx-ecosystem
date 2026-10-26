import type { Jogo } from "../types/jogo";

/** Servidor Axum do shmupx-backend. Configurável via env para apontar para outro ambiente. */
export const API_BASE_URL = process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://localhost:3000";

export class ApiError extends Error {
  constructor(
    message: string,
    public readonly status?: number,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

/**
 * `GET /api/jogos`: lista real de jogos cadastrados (ver shmupx-backend/src/routes/jogos.rs).
 * `cache: "no-store"` porque o catálogo pode mudar a qualquer momento (o Lab publica jogos
 * novos) — não queremos uma versão velha presa no cache estático do Next.
 */
export async function getJogos(): Promise<Jogo[]> {
  const response = await fetch(`${API_BASE_URL}/api/jogos`, { cache: "no-store" });

  if (!response.ok) {
    throw new ApiError(`GET /api/jogos retornou HTTP ${response.status}`, response.status);
  }

  return (await response.json()) as Jogo[];
}
