import { initializeApp } from "firebase/app";
import {
  browserLocalPersistence,
  GoogleAuthProvider,
  indexedDBLocalPersistence,
  initializeAuth,
  signInWithCredential,
  signInWithPopup,
  signOut,
  type User,
} from "firebase/auth";
import { invoke, isTauri } from "@tauri-apps/api/core";

/**
 * Configuração pública do app Web do projeto Firebase (a mesma do shmupx-client). Identifica o
 * projeto; não é credencial. A permissão de admin é decidida pelo backend a cada chamada.
 */
const app = initializeApp({
  projectId: "shmupx-project-id",
  appId: "1:230979440400:web:16c3094411b19dcecbdf95",
  apiKey: "AIzaSyB7ztOxpUY6wEnNUWUzD0wKT0aVr5HNDcM",
  authDomain: "shmupx-project-id.firebaseapp.com",
  messagingSenderId: "230979440400",
});

export const auth = initializeAuth(app, { persistence: [indexedDBLocalPersistence, browserLocalPersistence] });

/** Cliente OAuth "Desktop" do Google no mesmo projeto. Público, definido em `.env.local`. */
const DESKTOP_CLIENT_ID = import.meta.env.VITE_GOOGLE_DESKTOP_CLIENT_ID ?? "";

/**
 * Desktop: o Rust conduz o OAuth no navegador do sistema e devolve o id_token do Google.
 * Prévia no navegador: popup do próprio Firebase.
 */
export async function signInWithGoogle(): Promise<User> {
  if (!isTauri()) return (await signInWithPopup(auth, new GoogleAuthProvider())).user;
  if (!DESKTOP_CLIENT_ID) throw new Error("Login Google não configurado nesta build (VITE_GOOGLE_DESKTOP_CLIENT_ID).");
  const { id_token: idToken } = await invoke<{ id_token: string }>("login_google_desktop", { clientId: DESKTOP_CLIENT_ID });
  return (await signInWithCredential(auth, GoogleAuthProvider.credential(idToken))).user;
}

export function signOutAdmin() {
  return signOut(auth);
}
