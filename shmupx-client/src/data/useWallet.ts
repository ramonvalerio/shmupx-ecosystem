import { useEffect, useState } from "react";
import { doc, onSnapshot } from "firebase/firestore";
import { db } from "../auth/firebase";

/** Saldo de quem ainda não começou nenhuma run (`FICHAS_INICIAIS` no backend). */
const INITIAL_CREDITS = 50;

/**
 * Fichas do jogador: `wallets/{uid}.balance` (só o servidor grava; o jogador lê a própria). Cada
 * INICIAR JOGO de verdade gasta 1 (no PC e no MiSTer); o MODO TREINO não gasta. Atualiza na hora.
 */
export function useWallet(uid: string | undefined): number | null {
  const [balance, setBalance] = useState<number | null>(null);
  useEffect(() => {
    if (!uid) { setBalance(null); return; }
    return onSnapshot(
      doc(db, "wallets", uid),
      (snapshot) => {
        const value = snapshot.data()?.balance;
        setBalance(typeof value === "number" ? value : INITIAL_CREDITS);
      },
      () => setBalance(null),
    );
  }, [uid]);
  return balance;
}
