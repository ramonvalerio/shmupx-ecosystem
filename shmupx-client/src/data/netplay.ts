import { addDoc, collection, deleteDoc, doc, onSnapshot, query, serverTimestamp, updateDoc, where, type Timestamp } from "firebase/firestore";
import { invoke } from "@tauri-apps/api/core";
import { db } from "../auth/firebase";
import type { GameRoom } from "../types/room";
import { readEmulatorLibrary, romsFor } from "./emulatorLibrary";
import { readEmulatorSettings } from "./emulatorSettings";
import { readEmulatorControls } from "./emulatorControls";

/**
 * Treino em dupla online: clique direito num jogador da sala › Convidar para jogar. Os dois jogam o
 * mesmo jogo no ShmupX Emulator, cada um no seu PC (quem convidou é o jogador 1). Só no treino: não
 * vale ranking nem gasta ficha.
 *
 * Sessão em `netplaySessions/{id}` (só os dois leem):
 *   invited  → o anfitrião convidou;
 *   accepted → o convidado aceitou e publicou os endereços dele (`guest`);
 *   ready    → o anfitrião publicou os dele (`host`) e abriu o motor; o convidado abre o dele;
 *   declined / canceled / ended / failed → acabou (`active: false`, `reason`).
 * Os motores se conectam direto (UDP, rede local ou endereço público visto pelo STUN) e trocam só
 * os controles, com rollback (shmupx-engine/docs/NETPLAY.md).
 */

export type NetplayStatus = "invited" | "accepted" | "declined" | "canceled" | "ready" | "ended" | "failed";

export interface NetplaySide {
  port: number;
  candidates: string[];
}

export interface NetplaySession {
  id: string;
  hostUid: string;
  hostName: string;
  guestUid: string;
  guestName: string;
  gameId: string;
  title: string;
  key: string;
  status: NetplayStatus;
  active: boolean;
  createdAt: number;
  host?: NetplaySide;
  guest?: NetplaySide;
  reason?: string;
}

/** O convite vale por 1 minuto. */
export const INVITE_TTL_MS = 60_000;
/** Depois de aceito, o anfitrião tem esse tempo para abrir o motor. */
export const READY_TTL_MS = 45_000;

const COLLECTION = "netplaySessions";

/**
 * Jogos liberados para a dupla (ROMs): passam no SyncTest e no teste entre dois motores
 * (shmupx-engine/docs/NETPLAY.md). Só eles mostram o "Convidar para jogar".
 */
export const NETPLAY_ROMS = new Set<string>(["sonicwi2", "bgaregga", "cawing", "ddonpach", "batsugun", "grindstm"]);

export function netplayCertified(room: GameRoom): boolean {
  return romsFor(room).some((rom) => NETPLAY_ROMS.has(rom));
}

function sessionFrom(id: string, data: Record<string, unknown>): NetplaySession | null {
  if (typeof data.hostUid !== "string" || typeof data.guestUid !== "string" || typeof data.key !== "string" || typeof data.gameId !== "string") return null;
  const side = (value: unknown): NetplaySide | undefined => {
    const v = value as { port?: unknown; candidates?: unknown } | undefined;
    return v && typeof v.port === "number" && Array.isArray(v.candidates) ? { port: v.port, candidates: v.candidates.filter((c): c is string => typeof c === "string") } : undefined;
  };
  return {
    id,
    hostUid: data.hostUid,
    hostName: typeof data.hostName === "string" ? data.hostName : "Jogador",
    guestUid: data.guestUid,
    guestName: typeof data.guestName === "string" ? data.guestName : "Jogador",
    gameId: data.gameId,
    title: typeof data.title === "string" ? data.title : data.gameId,
    key: data.key,
    status: (typeof data.status === "string" ? data.status : "failed") as NetplayStatus,
    active: data.active === true,
    createdAt: (data.createdAt as Timestamp | null)?.toMillis?.() ?? Date.now(),
    host: side(data.host),
    guest: side(data.guest),
    reason: typeof data.reason === "string" ? data.reason : undefined,
  };
}

function randomKey(): string {
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/** Convida `guest` para jogar o jogo da sala em dupla. */
export async function createInvite(host: { uid: string; name: string }, guest: { uid: string; name: string }, room: GameRoom): Promise<string> {
  const ref = await addDoc(collection(db, COLLECTION), {
    hostUid: host.uid,
    hostName: host.name.slice(0, 64),
    guestUid: guest.uid,
    guestName: guest.name.slice(0, 64),
    gameId: room.id,
    title: room.title.slice(0, 128),
    key: randomKey(),
    status: "invited",
    active: true,
    createdAt: serverTimestamp(),
  });
  return ref.id;
}

export async function updateSession(id: string, change: Partial<Pick<NetplaySession, "status" | "active" | "reason" | "host" | "guest">>) {
  await updateDoc(doc(db, COLLECTION, id), { ...change, updatedAt: serverTimestamp() });
}

/** Encerra a sessão (sem erro se ela já tiver acabado ou sumido). */
export async function finishSession(id: string, status: Extract<NetplayStatus, "declined" | "canceled" | "ended" | "failed">, reason?: string) {
  await updateSession(id, { status, active: false, ...(reason ? { reason: reason.slice(0, 64) } : {}) }).catch((error) => console.warn("dupla: falha ao encerrar a sessão:", error));
}

export async function deleteSession(id: string) {
  await deleteDoc(doc(db, COLLECTION, id)).catch(() => {});
}

/** Sessões ativas em que o jogador é anfitrião ou convidado (tempo real). */
export function watchSessions(uid: string, onChange: (sessions: NetplaySession[]) => void): () => void {
  const lists: Record<"host" | "guest", NetplaySession[]> = { host: [], guest: [] };
  const emit = () => onChange([...lists.host, ...lists.guest]);
  const listen = (role: "host" | "guest") => onSnapshot(
    query(collection(db, COLLECTION), where(role === "host" ? "hostUid" : "guestUid", "==", uid), where("active", "==", true)),
    (snapshot) => {
      lists[role] = snapshot.docs.map((d) => sessionFrom(d.id, d.data())).filter((s): s is NetplaySession => s !== null);
      emit();
    },
    (error) => console.warn(`dupla: falha ao ouvir as sessões (${role}):`, error),
  );
  const stops = [listen("host"), listen("guest")];
  return () => stops.forEach((stop) => stop());
}

/** Porta UDP do motor e os endereços para o parceiro chegar nela (rede local e público pelo STUN). */
export async function prepareSide(): Promise<NetplaySide & { publicAddress: string | null }> {
  const result = await invoke<{ porta: number; candidatos: string[]; publico: string | null }>("dupla_preparar");
  return { port: result.porta, candidates: result.candidatos.slice(0, 6), publicAddress: result.publico };
}

/** Abre o jogo no ShmupX Emulator, no treino, conectado ao parceiro. */
export async function launchNetplay(room: GameRoom, role: "host" | "guest", mine: NetplaySide, partner: NetplaySide, key: string) {
  await invoke("inicializar_jogo_emulador", {
    jogoId: room.id,
    roms: romsFor(room),
    pasta: readEmulatorLibrary()?.root || null,
    treino: true,
    janela: false,
    motorVideo: readEmulatorSettings(),
    motorControles: readEmulatorControls(),
    dupla: { modo: role, porta: mine.port, parceiros: partner.candidates, chave: key },
  });
}

/** Evento do motor na dupla (`SHMUPX_NETPLAY {...}`). */
export interface NetplayEngineEvent {
  pid: number;
  evento: { event: string; reason?: string; rtt?: number; delay?: number; frame?: number; [key: string]: unknown };
}
