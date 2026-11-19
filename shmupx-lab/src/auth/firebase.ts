import { initializeApp, type FirebaseOptions } from "firebase/app";
import {
  browserLocalPersistence,
  connectAuthEmulator,
  GoogleAuthProvider,
  indexedDBLocalPersistence,
  initializeAuth,
  signInWithCredential,
  signInWithPopup,
  signOut,
  type Auth,
  type User,
} from "firebase/auth";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { backendUrlDe, type Ambiente } from "../lib/ambiente";

/**
 * Configuração pública do app Web do projeto Firebase (a mesma do shmupx-client). Identifica o
 * projeto; não é credencial. A permissão de admin é decidida pelo backend a cada chamada.
 */
const CONFIG: Record<Ambiente, FirebaseOptions> = {
  // Projeto "demo-*": o SDK só fala com os emuladores e nunca alcança recursos reais.
  dev: { projectId: "demo-shmupx", apiKey: "demo-key", authDomain: "demo-shmupx.firebaseapp.com" },
  prd: {
    projectId: "shmupx-project-id",
    appId: "1:230979440400:web:16c3094411b19dcecbdf95",
    apiKey: "AIzaSyB7ztOxpUY6wEnNUWUzD0wKT0aVr5HNDcM",
    authDomain: "shmupx-project-id.firebaseapp.com",
    messagingSenderId: "230979440400",
  },
};

const EMULADOR_AUTH = "http://127.0.0.1:9099";
const MENSAGEM_EMULADOR_FORA = "Emulador do Firebase não está rodando. Suba com docker compose em shmupx-infra/firebase.";

let atual: { ambiente: Ambiente; auth: Auth } | null = null;

/**
 * Inicia o Firebase só do ambiente escolhido na tela inicial. Chamar de novo com o mesmo
 * ambiente devolve a instância existente; trocar de ambiente exige recarregar o app.
 */
export function iniciarFirebase(ambiente: Ambiente): Auth {
  if (atual) {
    if (atual.ambiente !== ambiente) throw new Error("O Firebase já foi iniciado em outro ambiente. Recarregue o Lab.");
    return atual.auth;
  }
  const auth = initializeAuth(initializeApp(CONFIG[ambiente]), { persistence: [indexedDBLocalPersistence, browserLocalPersistence] });
  if (ambiente === "dev") connectAuthEmulator(auth, EMULADOR_AUTH, { disableWarnings: true });
  atual = { ambiente, auth };
  return auth;
}

function iniciado() {
  if (!atual) throw new Error("Firebase ainda não iniciado: escolha o ambiente primeiro.");
  return atual;
}

/** Ambiente escolhido nesta sessão do Lab. */
export function getAmbiente(): Ambiente {
  return iniciado().ambiente;
}

export function getLabAuth(): Auth {
  return iniciado().auth;
}

/** Endereço do shmupx-backend do ambiente escolhido (o mesmo que o Rust usa no login). */
export function getBackendUrl(): string {
  return backendUrlDe(getAmbiente());
}

/** Cliente OAuth "Desktop" do Google no mesmo projeto. Público, definido em `.env.local`. */
const DESKTOP_CLIENT_ID = import.meta.env.VITE_GOOGLE_DESKTOP_CLIENT_ID ?? "";

async function emuladorNoAr(): Promise<boolean> {
  try {
    await fetch(EMULADOR_AUTH, { mode: "no-cors", signal: AbortSignal.timeout(2000) });
    return true;
  } catch {
    return false;
  }
}

/**
 * Desktop: o Rust conduz o OAuth no navegador do sistema e devolve o id_token do Google.
 * Prévia no navegador: popup do próprio Firebase.
 */
export async function signInWithGoogle(): Promise<User> {
  const { ambiente, auth } = iniciado();
  // Em dev, falha cedo e com mensagem clara, antes de abrir o navegador.
  if (ambiente === "dev" && !(await emuladorNoAr())) throw new Error(MENSAGEM_EMULADOR_FORA);
  try {
    if (!isTauri()) return (await signInWithPopup(auth, new GoogleAuthProvider())).user;
    if (!DESKTOP_CLIENT_ID) throw new Error("Login Google não configurado nesta build (VITE_GOOGLE_DESKTOP_CLIENT_ID).");
    const { id_token: idToken } = await invoke<{ id_token: string }>("login_google_desktop", {
      clientId: DESKTOP_CLIENT_ID,
      backendUrl: backendUrlDe(ambiente),
    });
    return (await signInWithCredential(auth, GoogleAuthProvider.credential(idToken))).user;
  } catch (error) {
    const code = (error as { code?: unknown } | null)?.code;
    if (ambiente === "dev" && code === "auth/network-request-failed") throw new Error(MENSAGEM_EMULADOR_FORA);
    throw error;
  }
}

export function signOutAdmin() {
  return signOut(getLabAuth());
}

/** Sai da conta e recarrega: o Lab volta à tela de escolha do ambiente. */
export async function trocarAmbiente() {
  try {
    if (atual) await signOut(atual.auth);
  } finally {
    window.location.reload();
  }
}