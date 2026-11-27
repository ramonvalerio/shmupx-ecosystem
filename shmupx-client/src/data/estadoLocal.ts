import type { SessionRun } from "../types/submission";

/**
 * O que o Client guarda neste PC para nada se perder numa atualização:
 *
 * - **A tela**: seção, sala aberta (e a aba de Ranking ou Eventos dela), jogo selecionado, filtros da
 *   coleção, Configurações aberta e Big Picture, salvos a cada mudança. Logo antes de atualizar (o
 *   programa reinicia com o instalador novo, ou a interface nova recarrega), o Client marca a tela
 *   para voltar; no início seguinte, ela volta como estava. O que não existe mais (uma sala tirada do
 *   catálogo, uma seção que saiu) volta para Minhas coleções. Num início comum, o Client abre na
 *   Coleção, como sempre.
 * - **As partidas** da conta neste PC, sempre: a lista não some ao fechar ou atualizar o Client. Uma
 *   partida que estava gravando ou em análise quando o programa fechou volta como interrompida (no
 *   recarregar da interface, o programa continua com ela e nada muda).
 */

export interface SavedCollection { query: string; platform: string; library: string }

export interface SavedScreen {
  uid: string;
  section: string;
  activeRoomId: string | null;
  selectedRoomId: string | null;
  rankingRoomIds: string[];
  eventsRoomIds: string[];
  notificationId: string | null;
  settingsOpen: boolean;
  bigPicture: boolean;
  collection?: SavedCollection;
}

const SCREEN_KEY = "shmupx.client.tela.v1";
const RESTORE_KEY = "shmupx.client.tela.restaurar";
/** A marca de "voltar à tela" vale só para o início logo depois da atualização. */
const RESTORE_WINDOW_MS = 15 * 60_000;
const RUNS_KEY = (uid: string) => `shmupx.client.partidas.v1:${uid}`;
/** Partidas guardadas por conta (as mais recentes). */
export const MAX_SAVED_RUNS = 300;
/** Presente na sessão do navegador: some quando o programa fecha, fica num recarregar da interface. */
const SESSION_MARK = "shmupx.client.processo";

const isString = (value: unknown): value is string => typeof value === "string";
const stringOrNull = (value: unknown) => (isString(value) ? value : null);
const strings = (value: unknown) => (Array.isArray(value) ? value.filter(isString) : []);

/** Tela salva de `uid`, validada (campos estranhos viram o padrão). */
export function parseScreen(raw: string | null, uid: string): SavedScreen | null {
  let value: unknown;
  try { value = JSON.parse(raw ?? "null"); } catch { return null; }
  if (!value || typeof value !== "object") return null;
  const saved = value as Record<string, unknown>;
  if (saved.uid !== uid) return null;
  const collection = saved.collection as Record<string, unknown> | undefined;
  return {
    uid,
    section: isString(saved.section) ? saved.section : "collection",
    activeRoomId: stringOrNull(saved.activeRoomId),
    selectedRoomId: stringOrNull(saved.selectedRoomId),
    rankingRoomIds: strings(saved.rankingRoomIds),
    eventsRoomIds: strings(saved.eventsRoomIds),
    notificationId: stringOrNull(saved.notificationId),
    settingsOpen: saved.settingsOpen === true,
    bigPicture: saved.bigPicture === true,
    collection: collection && typeof collection === "object"
      ? { query: isString(collection.query) ? collection.query : "", platform: isString(collection.platform) ? collection.platform : "all", library: isString(collection.library) ? collection.library : "all" }
      : undefined,
  };
}

/** Salva parte da tela de `uid` (o resto fica como estava). */
export function saveScreen(partial: Partial<SavedScreen> & { uid: string }) {
  try {
    const current = parseScreen(localStorage.getItem(SCREEN_KEY), partial.uid);
    localStorage.setItem(SCREEN_KEY, JSON.stringify({ ...current, ...partial }));
  } catch { /* sem armazenamento: só não volta à tela depois de atualizar */ }
}

/** Antes de atualizar (reiniciar com o instalador novo ou recarregar a interface nova). */
export function markScreenForRestore() {
  try { localStorage.setItem(RESTORE_KEY, String(Date.now())); } catch { /* idem */ }
}

let restored: SavedScreen | null | undefined;

/**
 * A tela para voltar depois de uma atualização (uma vez por início do Client; `null` num início
 * comum). A marca é apagada ao ler: o início seguinte é comum.
 */
export function screenToRestore(uid: string): SavedScreen | null {
  if (restored === undefined) {
    try {
      const at = Number(localStorage.getItem(RESTORE_KEY));
      localStorage.removeItem(RESTORE_KEY);
      restored = at > 0 && Date.now() - at < RESTORE_WINDOW_MS ? parseScreen(localStorage.getItem(SCREEN_KEY), uid) : null;
    } catch {
      restored = null;
    }
  }
  return restored?.uid === uid ? restored : null;
}

const RUN_STATUSES: SessionRun["status"][] = ["recording", "analyzing", "submitted", "expired", "failed"];
export const INTERRUPTED_MESSAGE = "Interrompida: o ShmupX Client fechou antes de terminar a análise.";

/**
 * Partidas guardadas, validadas. `freshProcess`: o programa acabou de abrir (não é um recarregar da
 * interface), então o que estava gravando ou em análise não vai mais terminar: volta interrompida.
 */
export function parseRuns(raw: string | null, freshProcess: boolean): SessionRun[] {
  let value: unknown;
  try { value = JSON.parse(raw ?? "[]"); } catch { return []; }
  if (!Array.isArray(value)) return [];
  return value
    .filter((run): run is SessionRun => Boolean(run) && typeof run === "object" && isString(run.id) && isString(run.gameId)
      && typeof run.startedAt === "number" && RUN_STATUSES.includes(run.status))
    .slice(0, MAX_SAVED_RUNS)
    .map((run) => freshProcess && (run.status === "recording" || run.status === "analyzing")
      ? { ...run, status: "failed" as const, endedAt: run.endedAt ?? run.startedAt, message: INTERRUPTED_MESSAGE, interrupted: true }
      : run);
}

function freshProcess(): boolean {
  try {
    const fresh = !sessionStorage.getItem(SESSION_MARK);
    sessionStorage.setItem(SESSION_MARK, "1");
    return fresh;
  } catch {
    return true;
  }
}
const FRESH_PROCESS = typeof sessionStorage === "undefined" ? true : freshProcess();

/** Partidas desta conta neste PC. */
export function loadRuns(uid: string): SessionRun[] {
  try { return parseRuns(localStorage.getItem(RUNS_KEY(uid)), FRESH_PROCESS); } catch { return []; }
}

/** Guarda as partidas desta conta (as mais recentes primeiro na lista do Client). */
export function saveRuns(uid: string, runs: SessionRun[]) {
  try { localStorage.setItem(RUNS_KEY(uid), JSON.stringify(runs.slice(0, MAX_SAVED_RUNS))); } catch { /* sem espaço: ficam nesta sessão */ }
}
