import { useCallback, useEffect, useMemo, useState } from "react";
import { addDoc, collection, limit, onSnapshot, orderBy, query, serverTimestamp, Timestamp, where } from "firebase/firestore";
import { db } from "../auth/firebase";
import type { ChatMessage } from "../types/room";
import { censor } from "./profanity";
import { useHiddenChatAuthors, useMyModeration } from "./useModeration";

/** Aviso do chat para quem foi mutado no Users Manager do Lab. */
export const MUTED_MESSAGE = "Você está sem permissão para enviar mensagens no chat.";
/** Aviso no lugar do chat para quem perdeu a visibilidade no chat (Users Manager do Lab). */
export const OUT_OF_CHAT_MESSAGE = "O chat das salas não está disponível para a sua conta.";

/** Aviso no lugar do chat no modo Invisível (status no avatar). */
export const INVISIBLE_MESSAGE = "Você está invisível: o chat fica escondido. Para ver e participar do chat, mude o status para Online ou Ausente no seu avatar.";

/** Mensagens mais recentes mantidas na tela. */
const HISTORY = 100;
/** Limite de `validMessage` em firestore.rules. */
export const MAX_MESSAGE_LENGTH = 500;

export type ChatStatus = "loading" | "ready" | "error";

/** Folga para relógio do PC adiantado: a própria mensagem logo após entrar não some. */
const CLOCK_SLACK_MS = 30_000;

/**
 * Chat da sala em tempo real: `rooms/{gameId}/messages` no Firestore (Security Rules: só jogadores
 * logados, em jogos publicados, postando com o próprio uid e nome). Funciona igual com várias
 * instâncias do backend, porque não passa por ele.
 */
export function useRoomChat(roomId: string, author: { uid: string; displayName: string }, since?: number, enabled = true) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [status, setStatus] = useState<ChatStatus>("loading");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setMessages([]);
    setStatus("loading");
    setError(null);
    // Modo Invisível: nem escuta as mensagens.
    if (!enabled) return;
    // `since`: só as mensagens desde que o jogador entrou na sala (ver data/roomSession).
    const messages = collection(db, "rooms", roomId, "messages");
    const recent = since
      ? query(messages, where("createdAt", ">=", Timestamp.fromMillis(since - CLOCK_SLACK_MS)), orderBy("createdAt", "desc"), limit(HISTORY))
      : query(messages, orderBy("createdAt", "desc"), limit(HISTORY));
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
              // ISO: ordena entre dias e a tela formata no idioma do jogador.
              timestamp: (createdAt ?? new Date()).toISOString(),
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
  }, [roomId, since, enabled]);

  // Moderação (Lab): quem perdeu a visibilidade no chat some para os outros; mutado não envia.
  const hidden = useHiddenChatAuthors();
  const { muted, banned, disabled, chatHidden } = useMyModeration(author.uid);
  const silenced = muted || banned || disabled || chatHidden;
  const visible = useMemo(
    () => (hidden.size ? messages.filter((m) => !m.authorId || m.authorId === author.uid || !hidden.has(m.authorId)) : messages),
    [messages, hidden, author.uid],
  );

  const send = useCallback(
    async (text: string) => {
      if (!enabled) {
        setError(INVISIBLE_MESSAGE);
        throw new Error(INVISIBLE_MESSAGE);
      }
      if (silenced) {
        const message = chatHidden ? OUT_OF_CHAT_MESSAGE : MUTED_MESSAGE;
        setError(message);
        throw new Error(message);
      }
      // Palavrões viram `*` antes de gravar: todo mundo recebe a mensagem já limpa.
      const trimmed = censor(text.trim().slice(0, MAX_MESSAGE_LENGTH)).text;
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
    [roomId, author.uid, author.displayName, silenced, chatHidden, enabled],
  );

  // Fora do chat: as Rules recusam a leitura; no lugar das mensagens, o aviso.
  if (!enabled) return { messages: [], status: "error" as ChatStatus, error: INVISIBLE_MESSAGE, send };
  if (chatHidden) return { messages: [], status: "error" as ChatStatus, error: OUT_OF_CHAT_MESSAGE, send };
  return { messages: visible, status, error: silenced ? MUTED_MESSAGE : error, send };
}
