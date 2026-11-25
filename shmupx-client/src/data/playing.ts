import { useEffect, useRef, useSyncExternalStore } from "react";
import { deleteField, getDoc, setDoc } from "firebase/firestore";
import { remoteRef, type RemoteState } from "./remoteLaunch";

/**
 * Jogo aberto na conta: `remoteLaunch/{uid}.playing`. O Client grava os jogos que abre no PC, e o
 * agente do MiSTer (pelo backend) os que abrem no MiSTer, por qualquer caminho, e libera a conta na
 * volta ao menu. Um jogo valendo ranking (Ranked ou Live) por vez: antes de abrir outro, o Client e o
 * Lite conferem aqui. Training não trava a conta nem é travado (ver `blocksLaunch`).
 */
export interface PlayingNow {
  gameId: string;
  title: string;
  platform: "steam" | "emulator" | "mister";
  kind: "training" | "ranked" | "live";
  /** Quem marcou: o Client do PC, o Lite do celular ou o agente do próprio MiSTer. */
  device: "pc" | "lite" | "mister";
  /** ms desde a época Unix. */
  startedAt: number;
  /** Último sinal de quem abriu (renovado a cada minuto enquanto o jogo está aberto). */
  beatAt: number;
}

export const PLAYING_BEAT_MS = 60_000;
/** O Client renova sozinho: 3 min sem sinal = o PC fechou sem avisar. */
const PC_STALE_MS = 3 * 60_000;
/**
 * Jogo no MiSTer marcado pelo Client ou pelo Lite (antes de o agente do MiSTer assumir): vale por
 * pouco tempo. O agente marca de novo (`device: "mister"`) e renova a cada minuto até a volta ao menu.
 */
const MISTER_HANDOFF_MS = 5 * 60_000;

/** O registro ainda vale (quem abriu não sumiu). */
export function playingActive(playing: PlayingNow | undefined | null, now = Date.now()): playing is PlayingNow {
  if (!playing?.gameId) return false;
  const limit = playing.platform === "mister" && playing.device !== "mister" ? MISTER_HANDOFF_MS : PC_STALE_MS;
  return now - Math.max(playing.beatAt, playing.startedAt) < limit;
}

/**
 * O jogo aberto impede abrir este? Só um jogo valendo ranking por vez: Training nunca trava (nem é
 * travado), então dá para treinar à vontade e trocar de jogo sem fechar o anterior.
 */
export function blocksLaunch(open: PlayingNow | null | undefined, kind: PlayingNow["kind"]): open is PlayingNow {
  return Boolean(open) && open!.kind !== "training" && kind !== "training";
}

/** Jogo sem leitura de placar no MiSTer (flag "Ranked no MiSTer" do Lab desligada): Client e Lite. */
export const RANKED_MISTER_OFF = "No MiSTer, este jogo é só Training: o ShmupX ainda não lê o placar dele lá. Para Ranked, jogue no Emulador.";

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
    device: playing.device === "lite" ? "pelo celular" : playing.device === "mister" ? "no próprio MiSTer" : "no PC",
  };
}

/** Jogo aberto pelo próprio Client no MiSTer (o launcher avisa o App, que declara na conta). */
let misterFromClient: { gameId: string; title: string; kind: "training" | "ranked" } | null = null;
const misterListeners = new Set<() => void>();
export function setClientMisterGame(game: typeof misterFromClient) {
  misterFromClient = game;
  misterListeners.forEach((listener) => listener());
}
export const getClientMisterGame = () => misterFromClient;
export function useClientMisterGame() {
  return useSyncExternalStore((listener) => { misterListeners.add(listener); return () => misterListeners.delete(listener); }, () => misterFromClient);
}
