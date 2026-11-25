import { useEffect, useState } from "react";
import { doc, onSnapshot } from "firebase/firestore";
import { db } from "../auth/firebase";

/** Moderação da conta, decidida no Users Manager do Lab (`moderation/{uid}`, só o servidor grava). */
export interface Moderation {
  banned: boolean;
  disabled: boolean;
  muted: boolean;
  chatHidden: boolean;
}

const FREE: Moderation = { banned: false, disabled: false, muted: false, chatHidden: false };

/** Moderação do próprio jogador, em tempo real. Sem documento (ou sem ler) = conta liberada. */
export function useMyModeration(uid: string | null | undefined): Moderation {
  const [moderation, setModeration] = useState<Moderation>(FREE);
  useEffect(() => {
    if (!uid) { setModeration(FREE); return; }
    return onSnapshot(
      doc(db, "moderation", uid),
      (snapshot) => {
        const data = snapshot.data() ?? {};
        setModeration({ banned: data.banned === true, disabled: data.disabled === true, muted: data.muted === true, chatHidden: data.chatHidden === true });
      },
      () => setModeration(FREE),
    );
  }, [uid]);
  return moderation;
}

/** Quem não aparece no chat (sem visibilidade ou banido): `chatModeration/hidden.uids`. */
export function useHiddenChatAuthors(): ReadonlySet<string> {
  const [hidden, setHidden] = useState<ReadonlySet<string>>(() => new Set());
  useEffect(() => onSnapshot(
    doc(db, "chatModeration", "hidden"),
    (snapshot) => {
      const uids = snapshot.data()?.uids;
      setHidden(new Set(Array.isArray(uids) ? uids.filter((uid): uid is string => typeof uid === "string") : []));
    },
    () => setHidden(new Set()),
  ), []);
  return hidden;
}
