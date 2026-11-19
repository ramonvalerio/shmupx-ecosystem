import { getBackendUrl, getLabAuth } from "../auth/firebase";

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
  const user = getLabAuth().currentUser;
  // Segue o ambiente escolhido ao abrir o Lab (lib/ambiente.ts).
  const BACKEND_URL = getBackendUrl();
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

/** Antecedência entre o aviso e a liberação no Client. */
export type ReleaseLead = "urgent" | "24h" | "48h" | "7d";

/** Versão do ShmupX Client, como o backend devolve em `/api/admin/releases`. */
export interface ClientRelease {
  version: string;
  title: string;
  features: string[];
  fixes: string[];
  rules: string[];
  /** Scripts de validação incluídos, como `jogo@versão`. */
  scripts: string[];
  mandatory: boolean;
  /** `scheduled` vira `released` sozinho quando chega `release_at` (relógio do servidor). */
  status: "draft" | "scheduled" | "released" | "cancelled";
  lead?: ReleaseLead;
  scheduled_at?: string;
  release_at?: string;
  /** `not_configured`, `sending`, `sent` ou `failed: ...`. */
  email_status?: string;
  emails_sent?: number;
}

export type ClientReleaseInput = Pick<ClientRelease, "title" | "features" | "fixes" | "rules" | "scripts" | "mandatory">;

export function listReleases(): Promise<ClientRelease[]> {
  return adminRequest<ClientRelease[]>("/api/admin/releases");
}

/** Cria ou edita o rascunho da versão (versões agendadas não mudam). */
export function saveRelease(version: string, input: ClientReleaseInput): Promise<ClientRelease> {
  return adminRequest<ClientRelease>(`/api/admin/releases/${encodeURIComponent(version)}`, {
    method: "PUT",
    body: JSON.stringify(input),
  });
}

/** Agenda a versão: publica o aviso no Client, manda os e-mails e libera após a antecedência. */
export function scheduleRelease(version: string, lead: ReleaseLead): Promise<ClientRelease> {
  return adminRequest<ClientRelease>(`/api/admin/releases/${encodeURIComponent(version)}/schedule`, {
    method: "POST",
    body: JSON.stringify({ lead }),
  });
}

/** Cancela um agendamento ainda não liberado e esconde o aviso no Client. */
export function cancelRelease(version: string): Promise<ClientRelease> {
  return adminRequest<ClientRelease>(`/api/admin/releases/${encodeURIComponent(version)}/cancel`, { method: "POST" });
}
