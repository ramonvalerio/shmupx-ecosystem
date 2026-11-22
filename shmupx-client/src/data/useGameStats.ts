import { useEffect, useState } from "react";
import { collection, onSnapshot, query, where, type Timestamp } from "firebase/firestore";
import { db } from "../auth/firebase";

export interface GameStats {
  /** Runs valendo ranking começadas neste jogo (cada uma gastou 1 ficha). */
  credits: number;
  /** Tempo jogado nessas runs, em segundos (início ao fim registrados pelo servidor). */
  seconds: number;
}

const millis = (value: unknown) => (value && typeof (value as Timestamp).toMillis === "function" ? (value as Timestamp).toMillis() : null);

/**
 * Fichas gastas e tempo de jogo do jogador neste jogo: as gameplays dele (`gameplays`, que as
 * Security Rules só liberam para o dono). O treino não gasta ficha nem cria gameplay.
 */
export function useGameStats(uid: string | undefined, gameId: string): GameStats | null {
  const [stats, setStats] = useState<GameStats | null>(null);
  useEffect(() => {
    if (!uid) { setStats(null); return; }
    return onSnapshot(
      query(collection(db, "gameplays"), where("ownerId", "==", uid), where("gameId", "==", gameId)),
      (snapshot) => {
        let seconds = 0;
        for (const item of snapshot.docs) {
          const capture = (item.data().capture ?? {}) as Record<string, unknown>;
          const start = millis(capture.serverStartedAt);
          const end = millis(capture.serverEndedAt) ?? millis(item.data().updatedAt);
          const max = typeof capture.maxDurationSeconds === "number" ? capture.maxDurationSeconds : Infinity;
          if (start !== null && end !== null && end > start) seconds += Math.min((end - start) / 1000, max);
        }
        setStats({ credits: snapshot.size, seconds: Math.round(seconds) });
      },
      () => setStats(null),
    );
  }, [uid, gameId]);
  return stats;
}

/** 0min, 12min, 1h 05min. */
export function formatPlayTime(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  return hours > 0 ? `${hours}h ${String(minutes % 60).padStart(2, "0")}min` : `${minutes}min`;
}
