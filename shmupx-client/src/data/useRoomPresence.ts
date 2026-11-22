import { useEffect, useState } from "react";
import { collection, deleteDoc, doc, onSnapshot, serverTimestamp, setDoc, type Timestamp } from "firebase/firestore";
import { db } from "../auth/firebase";

/** De quanto em quanto tempo o Client confirma que continua conectado. */
const HEARTBEAT_MS = 30_000;
/** Sem confirmação há mais que isso (app fechado, sem rede), o jogador aparece offline. */
const STALE_MS = 75_000;
/** Reavalia quem ficou sem confirmar, mesmo sem mudança no Firestore. */
const TICK_MS = 15_000;

/** Verde: na sala, com o Client em primeiro plano. Vermelho: Client aberto, mas em outra tela ou
 * janela. Cinza: Client fechado ou sem conexão (continua na sala até sair pela aba). */
export type PresenceStatus = "online" | "away" | "offline";

export interface RoomPresence {
  uid: string;
  name: string;
  status: PresenceStatus;
  playing: boolean;
}

function presenceRef(roomId: string, uid: string) {
  return doc(db, "rooms", roomId, "presence", uid);
}

/** Sair da sala pela aba: o jogador deixa a lista. Fechar o app ou cair a conexão não tira. */
export function leaveRoomPresence(roomId: string, uid: string) {
  return deleteDoc(presenceRef(roomId, uid)).catch((failure) => console.warn("falha ao sair da sala:", failure));
}

/** Client em primeiro plano (janela com foco e visível). */
function useForeground() {
  const [foreground, setForeground] = useState(() => document.visibilityState === "visible" && document.hasFocus());
  useEffect(() => {
    const update = () => setForeground(document.visibilityState === "visible" && document.hasFocus());
    window.addEventListener("focus", update);
    window.addEventListener("blur", update);
    document.addEventListener("visibilitychange", update);
    return () => {
      window.removeEventListener("focus", update);
      window.removeEventListener("blur", update);
      document.removeEventListener("visibilitychange", update);
    };
  }, []);
  return foreground;
}

/**
 * Presença do jogador em todas as salas abertas nas abas: `rooms/{gameId}/presence/{uid}` no
 * Firestore (Security Rules: só o próprio jogador grava o seu, com o nome do perfil e o horário do
 * servidor). Confirma a cada 30 s e quando o estado muda: jogando, na sala em primeiro plano
 * (online) ou com o Client aberto em outro lugar (away).
 */
export function usePresenceHeartbeat(me: { uid: string; displayName: string }, openRoomIds: string[], visibleRoomId: string | null, recordingRoomId: string | null) {
  const foreground = useForeground();
  const rooms = openRoomIds.join("\n");
  useEffect(() => {
    if (!me.uid || !rooms) return;
    const ids = rooms.split("\n");
    const beat = () => {
      for (const roomId of ids) {
        const state = recordingRoomId === roomId ? "playing" : foreground && visibleRoomId === roomId ? "online" : "away";
        setDoc(presenceRef(roomId, me.uid), { name: me.displayName, state, lastSeen: serverTimestamp() }).catch((failure) =>
          console.warn("falha ao registrar a presença na sala:", failure),
        );
      }
    };
    beat();
    const timer = window.setInterval(beat, HEARTBEAT_MS);
    return () => window.clearInterval(timer);
  }, [me.uid, me.displayName, rooms, visibleRoomId, recordingRoomId, foreground]);
}

/** Os outros jogadores da sala (quem entrou e não saiu pela aba), com o status de cada um. */
export function useRoomPresence(roomId: string, myUid: string): RoomPresence[] {
  const [entries, setEntries] = useState<Array<{ uid: string; name: string; state: string; lastSeen: number }>>([]);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    setEntries([]);
    return onSnapshot(
      collection(db, "rooms", roomId, "presence"),
      (snapshot) =>
        setEntries(
          snapshot.docs.map((d) => {
            const data = d.data({ serverTimestamps: "estimate" });
            return {
              uid: d.id,
              name: typeof data.name === "string" ? data.name : "Jogador",
              state: typeof data.state === "string" ? data.state : "online",
              lastSeen: (data.lastSeen as Timestamp | null)?.toMillis() ?? Date.now(),
            };
          }),
        ),
      (failure) => console.warn("falha ao ler a presença da sala:", failure),
    );
  }, [roomId]);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), TICK_MS);
    return () => window.clearInterval(timer);
  }, []);

  // O próprio jogador a tela mostra sempre, pelo estado local.
  return entries
    .filter((e) => e.uid !== myUid)
    .map(({ uid, name, state, lastSeen }) => {
      const offline = now - lastSeen >= STALE_MS;
      const status: PresenceStatus = offline ? "offline" : state === "away" ? "away" : "online";
      return { uid, name, status, playing: !offline && state === "playing" };
    });
}
