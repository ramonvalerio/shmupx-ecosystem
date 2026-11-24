import { useCallback, useEffect, useState } from "react";
import { arrayRemove, arrayUnion, doc, getDoc, onSnapshot, serverTimestamp, setDoc } from "firebase/firestore";
import { db } from "../auth/firebase";
import type { ChatMessage } from "../types/room";

/**
 * Segurança do chat feita pelo próprio jogador (exigência da Google Play para apps com conversa entre
 * usuários): bloquear quem incomoda e denunciar uma mensagem. Vale no Client e no Lite, pela conta.
 * - `chatBlocks/{uid}.uids`: quem o jogador bloqueou. Só ele lê e grava; as mensagens somem só para ele.
 * - `chatReports/{messageId}__{uid}`: denúncia de uma mensagem. Só o servidor lê (moderação no Lab);
 *   o jogador cria uma por mensagem e as Rules conferem que a mensagem existe e é de quem foi denunciado.
 */

/** Limite de bloqueios por conta (o mesmo das Rules). */
export const MAX_BLOCKED = 500;

export type ReportReason = "abuse" | "spam" | "inappropriate" | "other";

/** Motivos da denúncia, na ordem do menu. */
export const REPORT_REASONS: { id: ReportReason; label: string }[] = [
  { id: "abuse", label: "Ofensa ou assédio" },
  { id: "spam", label: "Spam ou golpe" },
  { id: "inappropriate", label: "Conteúdo impróprio" },
  { id: "other", label: "Outro motivo" },
];

/** Quem o jogador bloqueou, em tempo real, e as ações de bloquear e desbloquear. */
export function useBlockedAuthors(uid: string | null | undefined) {
  const [blocked, setBlocked] = useState<ReadonlySet<string>>(() => new Set());
  useEffect(() => {
    if (!uid) { setBlocked(new Set()); return; }
    return onSnapshot(
      doc(db, "chatBlocks", uid),
      (snapshot) => {
        const uids = snapshot.data()?.uids;
        setBlocked(new Set(Array.isArray(uids) ? uids.filter((id): id is string => typeof id === "string") : []));
      },
      () => setBlocked(new Set()),
    );
  }, [uid]);

  const block = useCallback(async (authorId: string) => {
    if (!uid || authorId === uid) return;
    if (blocked.size >= MAX_BLOCKED) throw new Error(`Limite de ${MAX_BLOCKED} bloqueios.`);
    await setDoc(doc(db, "chatBlocks", uid), { uids: arrayUnion(authorId), updatedAt: serverTimestamp() }, { merge: true });
  }, [uid, blocked.size]);

  const unblock = useCallback(async (authorId: string) => {
    if (!uid) return;
    await setDoc(doc(db, "chatBlocks", uid), { uids: arrayRemove(authorId), updatedAt: serverTimestamp() }, { merge: true });
  }, [uid]);

  return { blocked, block, unblock };
}

/** Resultado da denúncia: enviada agora ou já enviada antes pelo mesmo jogador. */
export type ReportOutcome = "sent" | "already";

/** Denuncia uma mensagem da sala. Uma denúncia por mensagem e jogador. */
export async function reportMessage(reporterId: string, roomId: string, message: ChatMessage, reason: ReportReason): Promise<ReportOutcome> {
  if (!message.authorId) throw new Error("mensagem sem autor");
  const ref = doc(db, "chatReports", `${message.id}__${reporterId}`);
  if ((await getDoc(ref)).exists()) return "already";
  await setDoc(ref, {
    reporterId,
    authorId: message.authorId,
    authorName: message.author.slice(0, 64),
    roomId,
    messageId: message.id,
    text: message.text.slice(0, 500),
    reason,
    createdAt: serverTimestamp(),
  });
  return "sent";
}
