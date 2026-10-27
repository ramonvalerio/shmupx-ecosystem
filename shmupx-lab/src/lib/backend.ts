import { auth } from "../auth/firebase";

/** Endereço do shmupx-backend. O build de distribuição define `VITE_BACKEND_URL`. */
const BACKEND_URL: string = import.meta.env.VITE_BACKEND_URL ?? "http://localhost:3000";

/** Jogo do catálogo, como o backend devolve em `/api/admin/games`. */
export interface CatalogGame {
  id: string;
  title: string;
  steam_app_id: number;
  max_duration_segundos: number;
  published: boolean;
  developer?: string;
  publisher?: string;
  genre?: string;
  year?: number;
  /** Um de `PLATFORMS` (features/catalog/options.ts). */
  platform?: string;
  /** Subconjunto de `MODES`. */
  modes: string[];
}

/** Os metadados opcionais substituem os anteriores: o que não for enviado é apagado. */
export type CatalogGameInput = Omit<CatalogGame, "id">;

async function adminRequest<T>(path: string, init: RequestInit = {}): Promise<T> {
  const user = auth.currentUser;
  if (!user) throw new Error("Sem sessão: entre com sua conta Google.");
  let response: Response;
  try {
    response = await fetch(`${BACKEND_URL}${path}`, {
      ...init,
      headers: {
        ...init.headers,
        Authorization: `Bearer ${await user.getIdToken()}`,
        ...(init.body ? { "Content-Type": "application/json" } : {}),
      },
    });
  } catch {
    throw new Error(`Não foi possível falar com o backend em ${BACKEND_URL}. Ele está rodando?`);
  }
  if (response.status === 403) throw new Error("Esta conta não é administradora (ADMIN_EMAILS no backend).");
  if (!response.ok) {
    const body = await response.json().catch(() => null);
    throw new Error(body?.error ?? `Backend respondeu HTTP ${response.status}.`);
  }
  return response.json() as Promise<T>;
}

export function listGames(): Promise<CatalogGame[]> {
  return adminRequest<CatalogGame[]>("/api/admin/games");
}

/** Cria ou atualiza o jogo. `published: true` o libera ao Client; `false` o esconde. */
export function saveGame(id: string, input: CatalogGameInput): Promise<CatalogGame> {
  return adminRequest<CatalogGame>(`/api/admin/games/${encodeURIComponent(id)}`, {
    method: "PUT",
    body: JSON.stringify(input),
  });
}
