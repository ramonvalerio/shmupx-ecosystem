import { initializeApp } from "firebase/app";
import { browserLocalPersistence, connectAuthEmulator, GoogleAuthProvider, indexedDBLocalPersistence, initializeAuth, signInWithCredential, signInWithPopup, signOut, type User } from "firebase/auth";
import { connectFirestoreEmulator, doc, getFirestore, runTransaction, serverTimestamp, updateDoc } from "firebase/firestore";
import { isTauri, invoke } from "@tauri-apps/api/core";
import type { Language } from "../i18nMessages";

/**
 * Ambiente: `dev` usa os emuladores do Firebase em Docker (shmupx-infra/firebase, projeto
 * demo-shmupx; nada toca produção); `prd` usa o projeto real. Padrão: `dev` em `npm run dev` /
 * `tauri dev`, `prd` nos builds. `VITE_SHMUPX_ENV` força um dos dois.
 */
export const AMBIENTE: "dev" | "prd" = (import.meta.env.VITE_SHMUPX_ENV ?? (import.meta.env.DEV ? "dev" : "prd")) === "prd" ? "prd" : "dev";

/**
 * Configuração pública do app Web "ShmupX Client" (shmupx-infra/firebase/apps/shmupx-client.json).
 * Identifica o projeto; não é credencial. A proteção vem de Authentication, Rules e backend.
 */
const app = initializeApp(
  AMBIENTE === "dev"
    ? // Projeto "demo-*": o SDK só fala com os emuladores e nunca alcança recursos reais.
      { projectId: "demo-shmupx", apiKey: "demo-key", authDomain: "demo-shmupx.firebaseapp.com" }
    : {
        projectId: "shmupx-project-id",
        appId: "1:230979440400:web:16c3094411b19dcecbdf95",
        apiKey: "AIzaSyB7ztOxpUY6wEnNUWUzD0wKT0aVr5HNDcM",
        authDomain: "shmupx-project-id.firebaseapp.com",
        messagingSenderId: "230979440400",
      },
);

// Sessão persistida no IndexedDB da WebView: o jogador continua conectado ao reabrir o app.
export const auth = initializeAuth(app, { persistence: [indexedDBLocalPersistence, browserLocalPersistence] });
export const db = getFirestore(app);
if (AMBIENTE === "dev") {
  connectAuthEmulator(auth, "http://127.0.0.1:9099", { disableWarnings: true });
  connectFirestoreEmulator(db, "127.0.0.1", 8088);
}

/** Cliente OAuth "Desktop" do Google no mesmo projeto. Público, definido em `.env.local`. */
const DESKTOP_CLIENT_ID = import.meta.env.VITE_GOOGLE_DESKTOP_CLIENT_ID ?? "";

export interface PlayerProfile { uid: string; displayName: string; locale: Language; }

/**
 * Desktop: o Rust conduz o OAuth no navegador do sistema e devolve o id_token do Google.
 * Prévia no navegador: popup do próprio Firebase (localhost é domínio autorizado por padrão).
 */
export async function signInWithGoogle(): Promise<User> {
  if (!isTauri()) return (await signInWithPopup(auth, new GoogleAuthProvider())).user;
  if (!DESKTOP_CLIENT_ID) throw new Error("Login Google não configurado nesta build (VITE_GOOGLE_DESKTOP_CLIENT_ID).");
  const { id_token: idToken } = await invoke<{ id_token: string }>("login_google_desktop", { clientId: DESKTOP_CLIENT_ID });
  return (await signInWithCredential(auth, GoogleAuthProvider.credential(idToken))).user;
}

export function signOutPlayer() { return signOut(auth); }

/** Limites de `validUser` em firestore.rules: 1..32 caracteres. */
export function normalizeDisplayName(name: string | null | undefined): string {
  const trimmed = (name ?? "").trim().slice(0, 32).trim();
  return trimmed || "Jogador";
}

/**
 * Lê `users/{uid}` e o cria na primeira entrada, numa transação: dois dispositivos entrando ao
 * mesmo tempo não sobrescrevem o perfil. Os timestamps são do servidor, como as Rules exigem.
 */
export async function ensurePlayerProfile(user: User, locale: Language): Promise<PlayerProfile> {
  const ref = doc(db, "users", user.uid);
  return runTransaction(db, async (transaction) => {
    const snapshot = await transaction.get(ref);
    if (snapshot.exists()) {
      const data = snapshot.data();
      return { uid: user.uid, displayName: String(data.displayName), locale: data.locale === "en" ? "en" : "pt-BR" };
    }
    const profile: PlayerProfile = { uid: user.uid, displayName: normalizeDisplayName(user.displayName), locale };
    transaction.set(ref, { displayName: profile.displayName, locale: profile.locale, createdAt: serverTimestamp(), updatedAt: serverTimestamp() });
    return profile;
  });
}

export async function updateDisplayName(uid: string, displayName: string): Promise<void> {
  await updateDoc(doc(db, "users", uid), { displayName, updatedAt: serverTimestamp() });
}
