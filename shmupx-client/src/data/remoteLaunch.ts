import { useEffect, useState } from "react";
import { doc, onSnapshot, setDoc } from "firebase/firestore";
import { db } from "../auth/firebase";
import type { GameMode, PlayOn } from "../types/room";

/**
 * Jogar no PC pelo celular. O ShmupX Lite grava um pedido em `remoteLaunch/{uid}` (só o dono lê e
 * escreve) e o Client aberto no PC da mesma conta abre o jogo (Steam ou emulador), como o Play da
 * sala. O Client também avisa ali que está aberto e em quais plataformas tem cada jogo.
 */
export type RemotePlatform = Extract<PlayOn, "steam" | "emulator">;
export type RemoteKind = "training" | "ranked";

export interface RemoteCommand {
  id: string;
  action: "play" | "stop";
  gameId: string;
  platform?: RemotePlatform;
  kind?: RemoteKind;
  mode?: GameMode;
  /** ms desde a época Unix (relógio de quem pediu). */
  createdAt: number;
}

export interface RemoteResult {
  /** Id do pedido respondido. */
  id: string;
  status: "started" | "failed" | "stopped";
  message?: string;
  at: number;
}

export interface RemoteDesktop {
  /** Último sinal do Client aberto no PC (ms). */
  seenAt: number;
  /** Plataformas em que o PC tem cada jogo (bibliotecas importadas). */
  owned: Record<string, RemotePlatform[]>;
}

export interface RemoteState {
  desktop?: RemoteDesktop;
  command?: RemoteCommand;
  result?: RemoteResult;
  /** Jogo aberto na conta (um por vez): ver `playing.ts`. */
  playing?: import("./playing").PlayingNow;
}

/** O Client avisa a cada 45 s; mais de 2 min sem sinal = PC fechado. */
export const DESKTOP_HEARTBEAT_MS = 45_000;
export const DESKTOP_ONLINE_MS = 120_000;
/** Pedido mais velho que isso não é executado (o PC abriu depois). */
export const COMMAND_TTL_MS = 120_000;

export const remoteRef = (uid: string) => doc(db, "remoteLaunch", uid);

export function desktopOnline(desktop: RemoteDesktop | undefined, now = Date.now()): boolean {
  return Boolean(desktop && now - desktop.seenAt < DESKTOP_ONLINE_MS);
}

/** Estado do controle remoto da conta (PC aberto, último pedido e resposta). */
export function useRemoteLaunch(uid: string | null) {
  const [state, setState] = useState<RemoteState | null>(null);
  const [, tick] = useState(0);
  useEffect(() => {
    if (!uid) return;
    return onSnapshot(remoteRef(uid), (snapshot) => setState((snapshot.data() as RemoteState | undefined) ?? {}), () => setState({}));
  }, [uid]);
  // Reavalia "PC online" com o passar do tempo, mesmo sem mudança no documento.
  useEffect(() => {
    const timer = window.setInterval(() => tick((n) => n + 1), 15_000);
    return () => window.clearInterval(timer);
  }, []);
  return state;
}

/** Pede ao Client do PC para abrir (ou parar) um jogo. Devolve o id do pedido. */
export async function sendRemoteCommand(uid: string, command: Omit<RemoteCommand, "id" | "createdAt">): Promise<string> {
  const id = typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID() : `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const clean = Object.fromEntries(Object.entries(command).filter(([, value]) => value !== undefined));
  // `mergeFields`: troca o pedido inteiro, sem misturar com campos do anterior.
  await setDoc(remoteRef(uid), { command: { ...clean, id, createdAt: Date.now() } }, { mergeFields: ["command"] });
  return id;
}

/** Client do PC: está aberto e tem estes jogos. */
export function publishDesktop(uid: string, owned: Record<string, RemotePlatform[]>) {
  return setDoc(remoteRef(uid), { desktop: { seenAt: Date.now(), owned } }, { mergeFields: ["desktop"] });
}

/** Client do PC: resposta ao pedido. */
export function publishResult(uid: string, result: Omit<RemoteResult, "at">) {
  const clean = Object.fromEntries(Object.entries(result).filter(([, value]) => value !== undefined));
  return setDoc(remoteRef(uid), { result: { ...clean, at: Date.now() } }, { mergeFields: ["result"] });
}
