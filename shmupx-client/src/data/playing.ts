import { useEffect, useRef, useSyncExternalStore } from "react";
import { deleteField, getDoc, setDoc } from "firebase/firestore";
import { remoteRef, type RemoteState } from "./remoteLaunch";

/**
 * Um jogo por vez na conta. O jogo aberto fica em `remoteLaunch/{uid}.playing`: o Client grava os
 * jogos que abre (no PC ou no MiSTer) e o ShmupX Lite os que abre no MiSTer. Antes de abrir outro,
 * os dois conferem aqui e, se houver um aberto, mostram qual e oferecem PARAR JOGO.
 */
export interface PlayingNow {
  gameId: string;
  title: string;
  platform: "steam" | "emulator" | "mister";
  kind: "training" | "ranked" | "live";
  /** Quem abriu: o Client do PC ou o Lite do celular. */
  device: "pc" | "lite";
  /** ms desde a época Unix. */
  startedAt: number;
  /** Último sinal de quem abriu (renovado a cada minuto enquanto o jogo está aberto). */
  beatAt: number;
}

export const PLAYING_BEAT_MS = 60_000;
/** O Client renova sozinho: 3 min sem sinal = o PC fechou sem avisar. */
const PC_STALE_MS = 3 * 60_000;
/** O celular em segundo plano não renova; no MiSTer vale por até 4 h (ou até alguém parar). */
const MISTER_STALE_MS = 4 * 60 * 60_000;

/** O registro ainda vale (quem abriu não sumiu). */
export function playingActive(playing: PlayingNow | undefined | null, now = Date.now()): playing is PlayingNow {
  if (!playing?.gameId) return false;
  const limit = playing.platform === "mister" ? MISTER_STALE_MS : PC_STALE_MS;
  return now - Math.max(playing.beatAt, playing.startedAt) < limit;
}

/** Grava (ou apaga, com `null`) o jogo aberto da conta. */
export async function publishPlaying(uid: string, playing: PlayingNow | null) {
  await setDoc(remoteRef(uid), { playing: playing ?? deleteField() }, { mergeFields: ["playing"] });
}

/** Apaga o registro só se ele ainda for o deste jogo e deste aparelho (outro pode ter aberto depois). */
export async function releasePlaying(uid: string, gameId: string, device: PlayingNow["device"]) {
  const current = ((await getDoc(remoteRef(uid))).data() as RemoteState | undefined)?.playing;
  if (current && current.gameId === gameId && current.device === device) await publishPlaying(uid, null);
}

/** Libera a conta de qualquer jogo (PARAR JOGO de um jogo aberto em outro aparelho, ou registro velho). */
export const clearPlaying = (uid: string) => publishPlaying(uid, null);

/**
 * Enquanto `entry` existe, este aparelho declara o jogo aberto e renova o sinal a cada minuto; ao
 * virar `null`, libera a conta. `entry` é comparado por jogo, plataforma, modo e aparelho.
 */
export function usePlayingClaim(uid: string | null, entry: Omit<PlayingNow, "startedAt" | "beatAt"> | null) {
  const key = entry ? `${entry.gameId}|${entry.platform}|${entry.kind}|${entry.device}` : "";
  const latest = useRef(entry);
  latest.current = entry;
  useEffect(() => {
    const claim = latest.current;
    if (!uid || !claim) return;
    const startedAt = Date.now();
    const beat = () => publishPlaying(uid, { ...claim, startedAt, beatAt: Date.now() }).catch((error) => console.warn("jogo aberto: falha ao avisar a conta:", error));
    void beat();
    const timer = window.setInterval(beat, PLAYING_BEAT_MS);
    return () => {
      window.clearInterval(timer);
      releasePlaying(uid, claim.gameId, claim.device).catch((error) => console.warn("jogo aberto: falha ao liberar a conta:", error));
    };
  }, [uid, key]);
}

// Último `playing` lido da conta, para quem abre jogos sem ter o uid à mão (ex.: useGameLauncher).
let current: PlayingNow | null = null;
const listeners = new Set<() => void>();
export function setAccountPlaying(playing: PlayingNow | undefined | null) {
  const next = playing ?? null;
  if (JSON.stringify(next) === JSON.stringify(current)) return;
  current = next;
  listeners.forEach((listener) => listener());
}
export function getAccountPlaying(now = Date.now()): PlayingNow | null {
  return playingActive(current, now) ? current : null;
}
export function useAccountPlaying(): PlayingNow | null {
  const value = useSyncExternalStore((listener) => { listeners.add(listener); return () => listeners.delete(listener); }, () => current);
  return playingActive(value) ? value : null;
}

/** Onde está o jogo aberto, para as mensagens (ex.: "Ketsui · MiSTer · pelo celular"). */
export function playingWhere(playing: PlayingNow): { platform: string; device: string } {
  return {
    platform: playing.platform === "mister" ? "MiSTer" : playing.platform === "steam" ? "Steam" : "Emulador",
    device: playing.device === "lite" ? "pelo celular" : "no PC",
  };
}

/** Jogo aberto pelo próprio Client no MiSTer (o launcher avisa o App, que declara na conta). */
let misterFromClient: { gameId: string; title: string; kind: "training" | "ranked" } | null = null;
const misterListeners = new Set<() => void>();
export function setClientMisterGame(game: typeof misterFromClient) {
  misterFromClient = game;
  misterListeners.forEach((listener) => listener());
}
export function useClientMisterGame() {
  return useSyncExternalStore((listener) => { misterListeners.add(listener); return () => misterListeners.delete(listener); }, () => misterFromClient);
}
