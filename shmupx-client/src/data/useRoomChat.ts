import { useCallback, useEffect, useState } from "react";
import { addDoc, collection, limit, onSnapshot, orderBy, query, serverTimestamp, type Timestamp } from "firebase/firestore";
import { db } from "../auth/firebase";
import type { ChatMessage } from "../types/room";

/** Mensagens mais recentes mantidas na tela. */
const HISTORY = 100;
/** Limite de `validMessage` em firestore.rules. */
export const MAX_MESSAGE_LENGTH = 500;

export type ChatStatus = "loading" | "ready" | "error";

function formatTime(date: Date | null, locale: string): string {
  return (date ?? new Date()).toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });
}

/**
 * Chat da sala em tempo real: `rooms/{gameId}/messages` no Firestore (Security Rules: só jogadores
 * logados, em jogos publicados, postando com o próprio uid e nome). Funciona igual com várias
 * instâncias do backend, porque não passa por ele.
 */
export function useRoomChat(roomId: string, author: { uid: string; displayName: string }, locale: string) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [status, setStatus] = useState<ChatStatus>("loading");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setMessages([]);
    setStatus("loading");
    setError(null);
    const recent = query(collection(db, "rooms", roomId, "messages"), orderBy("createdAt", "desc"), limit(HISTORY));
    return onSnapshot(
      recent,
      (snapshot) => {
        const list = snapshot.docs
          .map((doc) => {
            // Mensagem própria ainda sem horário do servidor: usa a estimativa local.
            const data = doc.data({ serverTimestamps: "estimate" });
            const createdAt = (data.createdAt as Timestamp | null)?.toDate() ?? null;
            return {
              id: doc.id,
              author: typeof data.authorName === "string" ? data.authorName : "Jogador",
              authorId: typeof data.authorId === "string" ? data.authorId : undefined,
              text: typeof data.text === "string" ? data.text : "",
              timestamp: formatTime(createdAt, locale),
              pending: doc.metadata.hasPendingWrites,
            } satisfies ChatMessage;
          })
          .reverse();
        setMessages(list);
        setStatus("ready");
      },
      (failure) => {
        console.error("falha ao carregar o chat da sala:", failure);
        setStatus("error");
        setError(failure.code === "permission-denied" ? "Sem permissão para ler o chat desta sala." : "Não foi possível conectar ao chat da sala.");
      },
    );
  }, [roomId, locale]);

  const send = useCallback(
    async (text: string) => {
      const trimmed = text.trim().slice(0, MAX_MESSAGE_LENGTH);
      if (!trimmed) return;
      setError(null);
      try {
        await addDoc(collection(db, "rooms", roomId, "messages"), {
          authorId: author.uid,
          authorName: author.displayName,
          text: trimmed,
          createdAt: serverTimestamp(),
        });
      } catch (failure) {
        console.error("falha ao enviar mensagem:", failure);
        setError("A mensagem não foi enviada. Tente de novo.");
        throw failure;
      }
    },
    [roomId, author.uid, author.displayName],
  );

  return { messages, status, error, send };
}
