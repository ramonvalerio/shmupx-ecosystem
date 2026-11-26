import { useEffect, useState } from "react";
import { collection, doc, onSnapshot, query, where, type Timestamp } from "firebase/firestore";
import { db } from "../auth/firebase";

export interface GameStats {
  /** Runs valendo ranking começadas neste jogo (cada uma gastou 1 ficha). */
  credits: number;
  /** Tempo jogado nessas runs, em segundos (início ao fim registrados pelo servidor). */
  seconds: number;
  /** Quando começou a última run valendo ranking (ms), ou `null` sem nenhuma. */
  lastAt: number | null;
}

const millis = (value: unknown) => (value && typeof (value as Timestamp).toMillis === "function" ? (value as Timestamp).toMillis() : null);

/**
 * Fichas gastas e tempo de jogo do jogador neste jogo: os totais que o backend mantém em
 * `playerStats/{uid}_{gameId}` (o ranking online guarda só recordes, então as gameplays abaixo do
 * recorde saem do banco). Sem esse documento (o jogador ainda não jogou depois da mudança), conta
 * as gameplays dele como antes. As Security Rules só liberam os dois para o dono. O treino não gasta
 * ficha nem cria gameplay.
 */
export function useGameStats(uid: string | undefined, gameId: string): GameStats | null {
  const [totals, setTotals] = useState<GameStats | null | undefined>(undefined);
  const [fromGameplays, setFromGameplays] = useState<GameStats | null>(null);
  useEffect(() => {
    if (!uid) { setTotals(null); return; }
    return onSnapshot(
      doc(db, "playerStats", `${uid}_${gameId}`),
      (snapshot) => {
        if (!snapshot.exists()) { setTotals(null); return; }
        const data = snapshot.data();
        setTotals({
          credits: typeof data.credits === "number" ? data.credits : 0,
          seconds: typeof data.seconds === "number" ? data.seconds : 0,
          lastAt: millis(data.lastStartedAt),
        });
      },
      () => setTotals(null),
    );
  }, [uid, gameId]);
  useEffect(() => {
    // Só sem os totais do backend.
    if (!uid || totals !== null) { setFromGameplays(null); return; }
    return onSnapshot(
      query(collection(db, "gameplays"), where("ownerId", "==", uid), where("gameId", "==", gameId)),
      (snapshot) => {
        let seconds = 0;
        let lastAt: number | null = null;
        for (const item of snapshot.docs) {
          const capture = (item.data().capture ?? {}) as Record<string, unknown>;
          const start = millis(capture.serverStartedAt);
          const end = millis(capture.serverEndedAt) ?? millis(item.data().updatedAt);
          const max = typeof capture.maxDurationSeconds === "number" ? capture.maxDurationSeconds : Infinity;
          if (start !== null && end !== null && end > start) seconds += Math.min((end - start) / 1000, max);
          const began = start ?? millis(item.data().createdAt) ?? millis(item.data().updatedAt);
          if (began !== null && (lastAt === null || began > lastAt)) lastAt = began;
        }
        setFromGameplays({ credits: snapshot.size, seconds: Math.round(seconds), lastAt });
      },
      () => setFromGameplays(null),
    );
  }, [uid, gameId, totals]);
  return totals ?? fromGameplays;
}

/** Último ranked: só a hora se foi hoje; senão a data e a hora ("24/11/2026 21:14"). */
export function formatLastPlayed(timestamp: number, locale: string, now = new Date()): string {
  const date = new Date(timestamp);
  const time = date.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit", hour12: false });
  if (date.toDateString() === now.toDateString()) return time;
  return `${date.toLocaleDateString(locale, { day: "2-digit", month: "2-digit", year: "numeric" })} ${time}`;
}

/** 0min, 12min, 1h 05min. */
export function formatPlayTime(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  return hours > 0 ? `${hours}h ${String(minutes % 60).padStart(2, "0")}min` : `${minutes}min`;
}
