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
  /** Plataforma original (legado: um de `PLATFORMS`); o Lab não edita mais, só preserva. */
  platform?: string;
  /** Onde se joga no ShmupX: subconjunto de `PLAY_ON` (steam, emulator, mister). */
  platforms?: string[];
  /** Subconjunto de `MODES`. */
  modes: string[];
  /** Imagem do card no Client (WebP 16:9 em www.shmupx.com/catalog/covers/). */
  cover_url?: string;
  /** Ranked no MiSTer liberado: o agente do MiSTer lê o placar da memória do jogo. Exige MiSTer em `platforms`. */
  ranked_mister?: boolean;
  /** Data de cadastro (ISO 8601), definida pelo backend. */
  created_at?: string;
  /** Última alteração, inclusive no RobotX (ISO 8601), definida pelo backend. */
  updated_at?: string;
}

/** Os metadados opcionais substituem os anteriores: o que não for enviado é apagado. */
export type CatalogGameInput = Omit<CatalogGame, "id" | "created_at" | "updated_at">;

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

/** Cadastra um jogo novo. O backend recusa um ID que já existe, sem sobrescrever o jogo. */
export function createGame(id: string, input: CatalogGameInput): Promise<CatalogGame> {
  return adminRequest<CatalogGame>(`/api/admin/games/${encodeURIComponent(id)}`, {
    method: "POST",
    body: JSON.stringify(input),
  });
}

/** Exclui o jogo do catálogo (some do Client na hora). Runs e rankings já gravados ficam guardados. */
export async function deleteGame(id: string): Promise<void> {
  await adminRequest<{ id: string }>(`/api/admin/games/${encodeURIComponent(id)}`, { method: "DELETE" });
}

/** Atualiza um jogo existente. `published: true` o libera ao Client; `false` o esconde. */
export function saveGame(id: string, input: CatalogGameInput): Promise<CatalogGame> {
  return adminRequest<CatalogGame>(`/api/admin/games/${encodeURIComponent(id)}`, {
    method: "PUT",
    body: JSON.stringify(input),
  });
}

/** Pergunta e resposta curadas do RobotX (respondidas no Client, sem IA). */
export interface RobotxItem {
  question: string;
  answer: string;
}

export function getRobotx(gameId: string): Promise<RobotxItem[]> {
  return adminRequest<RobotxItem[]>(`/api/admin/games/${encodeURIComponent(gameId)}/robotx`);
}

/** Substitui as perguntas do jogo; vale na hora para os jogadores (jogo publicado). */
export function saveRobotx(gameId: string, items: RobotxItem[]): Promise<RobotxItem[]> {
  return adminRequest<RobotxItem[]>(`/api/admin/games/${encodeURIComponent(gameId)}/robotx`, {
    method: "PUT",
    body: JSON.stringify({ items }),
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
  /** `not_configured`, `sending`, `sent`, `skipped` ou `failed: ...`. */
  email_status?: string;
  emails_sent?: number;
  /** Itens gerados pelas alterações no Lab (só em rascunho); juntam-se ao texto ao agendar. */
  auto: ReleaseAutoItem[];
  installer?: ReleaseInstaller;
  /** Liberada, o Client baixa e instala sozinho. */
  auto_update: boolean;
  /** Sem instalador: só notas (mudanças de catálogo), não pede atualização do Client. */
  notes_only: boolean;
}

export type ReleaseSection = "features" | "fixes" | "rules" | "scripts";

export interface ReleaseAutoItem {
  /** Origem estável (ex.: `game:earthion:sala`): repetir a alteração atualiza o item. */
  key: string;
  section: ReleaseSection;
  text: string;
}

export interface ReleaseInstaller {
  url: string;
  signature: string;
  size?: number;
}

export type ClientReleaseInput = Pick<ClientRelease, "title" | "features" | "fixes" | "rules" | "scripts" | "mandatory">;

/** Rascunho aberto da próxima versão (o backend cria um se não houver). */
export function getOpenDraft(): Promise<ClientRelease> {
  return adminRequest<ClientRelease>("/api/admin/releases/draft");
}

/** Registra no rascunho aberto uma alteração feita no Lab; `text: null` retira o item. */
export function recordReleaseChange(key: string, section: ReleaseSection, text: string | null): Promise<ClientRelease> {
  return adminRequest<ClientRelease>("/api/admin/releases/draft/changes", {
    method: "POST",
    body: JSON.stringify({ key, section, text }),
  });
}

/** Tira um item automático do rascunho. */
export function removeAutoItem(version: string, key: string): Promise<ClientRelease> {
  return adminRequest<ClientRelease>(`/api/admin/releases/${encodeURIComponent(version)}/auto/${encodeURIComponent(key)}`, { method: "DELETE" });
}

/** Liga o instalador já publicado no S3 ao rascunho. */
export function setReleaseInstaller(version: string, installer: ReleaseInstaller): Promise<ClientRelease> {
  return adminRequest<ClientRelease>(`/api/admin/releases/${encodeURIComponent(version)}/installer`, {
    method: "PUT",
    body: JSON.stringify(installer),
  });
}

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

export interface ScheduleOptions {
  /** Clients baixam e instalam sozinhos ao liberar (exige instalador). */
  autoUpdate: boolean;
  /** E-mail aos jogadores que aceitam avisos. */
  sendEmail: boolean;
}

/** Agenda a versão: publica o aviso no Client, manda os e-mails (se pedido) e libera após a antecedência. */
export function scheduleRelease(version: string, lead: ReleaseLead, options: ScheduleOptions): Promise<ClientRelease> {
  return adminRequest<ClientRelease>(`/api/admin/releases/${encodeURIComponent(version)}/schedule`, {
    method: "POST",
    body: JSON.stringify({ lead, auto_update: options.autoUpdate, send_email: options.sendEmail }),
  });
}

/** Motivo de uma distribuição de fichas (o backend aceita só estes). */
export type CoinGrantReason =
  | "event_prize"
  | "season_reward"
  | "compensation"
  | "support_refund"
  | "promotion"
  | "welcome_bonus"
  | "bug_report"
  | "community"
  | "content_creator"
  | "other";

/** Distribuição de fichas do Coins Manager, como o backend devolve em `/api/admin/coins/grants`. */
export interface CoinGrant {
  id: string;
  target: "all" | "user";
  uid?: string;
  user_label?: string;
  amount: number;
  reason: CoinGrantReason;
  note?: string;
  scheduled_at: string;
  /** `scheduled` vira `done` sozinho quando chega a hora (o backend credita no próximo acesso). */
  status: "scheduled" | "running" | "done" | "cancelled" | "failed";
  created_by: string;
  created_at: string;
  executed_at?: string;
  recipients: number;
  error?: string;
}

export interface CoinGrantInput {
  target: "all" | "user";
  uid?: string;
  amount: number;
  reason: CoinGrantReason;
  note?: string;
  /** Ausente = agora. */
  scheduled_at?: string;
}

/** Lista as distribuições; o backend credita antes as que já venceram. */
export function listCoinGrants(): Promise<CoinGrant[]> {
  return adminRequest<CoinGrant[]>("/api/admin/coins/grants");
}

/** Cria a distribuição. Sem `scheduled_at`, credita na hora. */
export function createCoinGrant(input: CoinGrantInput): Promise<CoinGrant> {
  return adminRequest<CoinGrant>("/api/admin/coins/grants", { method: "POST", body: JSON.stringify(input) });
}

export function cancelCoinGrant(id: string): Promise<CoinGrant> {
  return adminRequest<CoinGrant>(`/api/admin/coins/grants/${encodeURIComponent(id)}/cancel`, { method: "POST" });
}

/** Roda de novo uma distribuição que falhou; quem já recebeu não recebe outra vez. */
export function retryCoinGrant(id: string): Promise<CoinGrant> {
  return adminRequest<CoinGrant>(`/api/admin/coins/grants/${encodeURIComponent(id)}/retry`, { method: "POST" });
}

export interface UserModeration {
  banned: boolean;
  muted: boolean;
  disabled: boolean;
  chat_hidden: boolean;
  note?: string;
  updated_at?: string;
  updated_by?: string;
}

export type UserStatus = "live" | "playing" | "online" | "away" | "offline";

/** Jogador cadastrado, como o backend devolve em `/api/admin/users`. */
export interface AdminUser {
  uid: string;
  display_name?: string;
  handle?: string;
  email?: string;
  created_at?: string;
  last_seen_at?: string;
  client_version?: string;
  /** Ausente: ainda sem carteira (ganha as fichas iniciais na primeira run). */
  balance?: number;
  status: UserStatus;
  /** Jogo aberto ou sala em que está. */
  activity?: string;
  is_admin: boolean;
  moderation: UserModeration;
  /** Denúncias do chat ainda abertas contra o jogador. */
  reports_open: number;
  /** Última alteração do perfil. */
  updated_at?: string;
  /** Tempo total com o ShmupX aberto (online, ausente, jogando ou ao vivo), contado desde 25/11/2026. */
  connected_seconds: number;
}

export type DownloadPlatform = "windows" | "appimage" | "deb" | "android";

/** Client Manager: cliques nos instaladores do site, visitantes anônimos e versões em uso. */
export interface ClientOverview {
  /** Cliques por dia (AAAA-MM-DD, horário de Brasília), só os dias com clique. */
  days: { day: string; clicks: Partial<Record<DownloadPlatform, number>> }[];
  /** Um item por visitante anônimo: os dias em que clicou. */
  visitors: string[][];
  versions: { version: string; players: number; active_7d: number }[];
  downloads: Partial<Record<"windows" | "appimage" | "deb", { version: string; url: string; size?: number }>>;
}

export function getClientOverview(): Promise<ClientOverview> {
  return adminRequest<ClientOverview>("/api/admin/client/overview");
}

export type ChatReportReason = "abuse" | "spam" | "inappropriate" | "other";

/** Denúncia de uma mensagem do chat (feita no Client ou no Lite), como o backend devolve em `/api/admin/reports`. */
export interface ChatReport {
  id: string;
  reporter_id: string;
  reporter_name?: string;
  author_id: string;
  author_name: string;
  room_id: string;
  room_title?: string;
  message_id: string;
  text: string;
  reason: ChatReportReason;
  created_at?: string;
  status: "open" | "actioned" | "dismissed";
  resolved_at?: string;
  resolved_by?: string;
}

export function listReports(): Promise<ChatReport[]> {
  return adminRequest<ChatReport[]>("/api/admin/reports");
}

/** `actioned` = houve moderação; `dismissed` = sem ação; `open` = reabrir. */
export function resolveReport(id: string, resolution: ChatReport["status"]): Promise<ChatReport> {
  return adminRequest<ChatReport>(`/api/admin/reports/${encodeURIComponent(id)}/resolve`, { method: "POST", body: JSON.stringify({ resolution }) });
}

export type UserModerationInput = Partial<Pick<UserModeration, "banned" | "muted" | "disabled" | "chat_hidden" | "note">>;

export function listUsers(): Promise<AdminUser[]> {
  return adminRequest<AdminUser[]>("/api/admin/users");
}

/** Só os campos enviados mudam. */
export function moderateUser(uid: string, input: UserModerationInput): Promise<UserModeration> {
  return adminRequest<UserModeration>(`/api/admin/users/${encodeURIComponent(uid)}/moderation`, { method: "PUT", body: JSON.stringify(input) });
}

/** Cancela um agendamento ainda não liberado e esconde o aviso no Client. */
export function cancelRelease(version: string): Promise<ClientRelease> {
  return adminRequest<ClientRelease>(`/api/admin/releases/${encodeURIComponent(version)}/cancel`, { method: "POST" });
}

/** Período de cobrança do plano. */
export type PlanPeriod = "monthly" | "quarterly" | "semiannual" | "yearly";

/** Plano de assinatura (`plans/{id}`). Preço em centavos de real. */
export interface Plan {
  id: string;
  name: string;
  description: string;
  price_cents: number;
  period: PlanPeriod;
  coins_per_period: number;
  benefits: string[];
  active: boolean;
  order: number;
  created_at: string | null;
  updated_at: string | null;
}
export type PlanInput = Omit<Plan, "id" | "created_at" | "updated_at">;

export function listPlans(): Promise<Plan[]> {
  return adminRequest<Plan[]>("/api/admin/plans");
}
/** Cria ou atualiza o plano. */
export function savePlan(id: string, input: PlanInput): Promise<Plan> {
  return adminRequest<Plan>(`/api/admin/plans/${encodeURIComponent(id)}`, { method: "PUT", body: JSON.stringify(input) });
}
export async function deletePlan(id: string): Promise<void> {
  await adminRequest<{ id: string }>(`/api/admin/plans/${encodeURIComponent(id)}`, { method: "DELETE" });
}

/** Modo que vale no evento (as tabelas do ranking). */
export type EventMode = "highscore" | "1cc" | "caravan";

/** Evento ou torneio (`events/{id}`). `published` o mostra no Client. */
export interface LabEvent {
  id: string;
  title: string;
  description: string;
  game_ids: string[];
  mode: EventMode;
  starts_at: string | null;
  ends_at: string | null;
  rules: string[];
  prizes: string[];
  published: boolean;
  created_at: string | null;
  updated_at: string | null;
}
export interface LabEventInput {
  title: string;
  description: string;
  game_ids: string[];
  mode: EventMode;
  starts_at: string;
  ends_at: string;
  rules: string[];
  prizes: string[];
  published: boolean;
}

export function listEvents(): Promise<LabEvent[]> {
  return adminRequest<LabEvent[]>("/api/admin/events");
}
/** Cria ou atualiza o evento. Os jogos precisam estar no catálogo. */
export function saveEvent(id: string, input: LabEventInput): Promise<LabEvent> {
  return adminRequest<LabEvent>(`/api/admin/events/${encodeURIComponent(id)}`, { method: "PUT", body: JSON.stringify(input) });
}
export async function deleteEvent(id: string): Promise<void> {
  await adminRequest<{ id: string }>(`/api/admin/events/${encodeURIComponent(id)}`, { method: "DELETE" });
}
