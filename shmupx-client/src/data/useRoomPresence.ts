import { useEffect, useState } from "react";
import { collection, deleteDoc, doc, onSnapshot, serverTimestamp, setDoc, type Timestamp } from "firebase/firestore";
import { db } from "../auth/firebase";

/** De quanto em quanto tempo o Client confirma que continua na sala. */
const HEARTBEAT_MS = 30_000;
/** Sem confirmação há mais que isso (app fechado à força, sem rede), o jogador some da lista. */
const STALE_MS = 75_000;
/** Reavalia quem ficou sem confirmar, mesmo sem mudança no Firestore. */
const TICK_MS = 15_000;

export interface RoomPresence {
  uid: string;
  name: string;
  playing: boolean;
}

/**
 * Presença em tempo real na sala: `rooms/{gameId}/presence/{uid}` no Firestore (Security Rules: só
 * o próprio jogador grava o seu documento, com o nome do perfil e o horário do servidor). O Client
 * confirma a presença a cada 30 s enquanto a sala está aberta e apaga o documento ao sair; quem
 * fica 75 s sem confirmar sai da lista.
 */
export function useRoomPresence(roomId: string, me: { uid: string; displayName: string }, playing: boolean) {
  const [entries, setEntries] = useState<Array<RoomPresence & { lastSeen: number }>>([]);
  const [now, setNow] = useState(() => Date.now());

  // Própria presença: grava ao entrar, a cada 30 s e quando muda o estado (jogando ou não).
  useEffect(() => {
    if (!me.uid) return;
    const ref = doc(db, "rooms", roomId, "presence", me.uid);
    const beat = () =>
      setDoc(ref, { name: me.displayName, state: playing ? "playing" : "online", lastSeen: serverTimestamp() }).catch((failure) =>
        console.warn("falha ao registrar a presença na sala:", failure),
      );
    void beat();
    const timer = window.setInterval(beat, HEARTBEAT_MS);
    return () => window.clearInterval(timer);
  }, [roomId, me.uid, me.displayName, playing]);

  // Ao sair da sala (ou fechar o app), apaga a presença.
  useEffect(() => {
    if (!me.uid) return;
    const ref = doc(db, "rooms", roomId, "presence", me.uid);
    const leave = () => void deleteDoc(ref).catch(() => undefined);
    window.addEventListener("beforeunload", leave);
    return () => {
      window.removeEventListener("beforeunload", leave);
      leave();
    };
  }, [roomId, me.uid]);

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
              playing: data.state === "playing",
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

  // Os outros jogadores ativos; o próprio jogador a tela mostra sempre, pelo estado local.
  return entries.filter((e) => e.uid !== me.uid && now - e.lastSeen < STALE_MS).map(({ uid, name, playing }) => ({ uid, name, playing }));
}
