import { initializeApp } from "firebase/app";
import { browserLocalPersistence, connectAuthEmulator, createUserWithEmailAndPassword, GoogleAuthProvider, indexedDBLocalPersistence, initializeAuth, linkWithCredential, sendEmailVerification, sendPasswordResetEmail, signInWithCredential, signInWithEmailAndPassword, signInWithPopup, signOut, updateProfile, type AuthCredential, type User } from "firebase/auth";
import { FirebaseError } from "firebase/app";
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
  if (!isTauri()) {
    try { return (await signInWithPopup(auth, new GoogleAuthProvider())).user; }
    catch (error) { throw pendingLinkOr(error, GoogleAuthProvider.credentialFromError(error as FirebaseError), ""); }
  }
  if (!DESKTOP_CLIENT_ID) throw new Error("Login Google não configurado nesta build (VITE_GOOGLE_DESKTOP_CLIENT_ID).");
  const { id_token: idToken } = await invoke<{ id_token: string }>("login_google_desktop", { clientId: DESKTOP_CLIENT_ID });
  const credential = GoogleAuthProvider.credential(idToken);
  try { return (await signInWithCredential(auth, credential)).user; }
  catch (error) { throw pendingLinkOr(error, credential, emailDoIdToken(idToken)); }
}

/**
 * O Google entrou com um e-mail que já tem cadastro por senha. Para e-mails do Gmail o Firebase
 * associa sozinho (o Google é o dono desses endereços); para os demais domínios ele pede que o
 * jogador prove a posse do cadastro uma vez, com a senha, e só então liga o Google à mesma conta.
 */
export class PendingGoogleLink extends Error {
  readonly email: string;
  readonly credential: AuthCredential;
  constructor(email: string, credential: AuthCredential) {
    super("account-exists-with-different-credential");
    this.email = email;
    this.credential = credential;
  }
}

function pendingLinkOr(error: unknown, credential: AuthCredential | null, fallbackEmail: string): unknown {
  if (!(error instanceof FirebaseError) || error.code !== "auth/account-exists-with-different-credential" || !credential) return error;
  const email = String((error.customData as { email?: string } | undefined)?.email ?? fallbackEmail);
  return new PendingGoogleLink(email, credential);
}

function emailDoIdToken(idToken: string): string {
  try { return String(JSON.parse(atob(idToken.split(".")[1].replace(/-/g, "+").replace(/_/g, "/"))).email ?? ""); }
  catch { return ""; }
}

/** Confirma a senha do cadastro existente e liga a ele o Google que o jogador acabou de usar. */
export async function linkGoogleWithPassword(email: string, password: string, credential: AuthCredential): Promise<void> {
  const { user } = await signInWithEmailAndPassword(auth, email, password);
  await linkWithCredential(user, credential);
  // A sessão passa a ser a do Google: o e-mail fica comprovado mesmo que o link de confirmação
  // nunca tenha sido aberto, e o backend aceita o token sem pedir a confirmação.
  await signInWithCredential(auth, credential).catch(() => user.reload());
}

/** Idioma dos e-mails de confirmação e de nova senha (modelos padrão do Firebase). */
function emailLanguage(locale: Language) { auth.languageCode = locale === "en" ? "en" : "pt-BR"; }

export function normalizeEmail(email: string): string { return email.trim().toLowerCase(); }

/** Formato de e-mail; a posse do endereço é comprovada pelo link de confirmação. */
export function looksLikeEmail(email: string): boolean { return /^[^\s@]+@[^\s@.]+(\.[^\s@.]+)+$/.test(normalizeEmail(email)); }

export const MIN_PASSWORD = 8;

/** Cadastro por e-mail e senha: o nome vai no perfil e o link de confirmação segue na hora. */
export async function signUpWithEmail(displayName: string, email: string, password: string, locale: Language): Promise<void> {
  const { user } = await createUserWithEmailAndPassword(auth, normalizeEmail(email), password);
  await updateProfile(user, { displayName: normalizeDisplayName(displayName) });
  emailLanguage(locale);
  await sendEmailVerification(user);
}

export async function signInWithEmail(email: string, password: string): Promise<void> {
  await signInWithEmailAndPassword(auth, normalizeEmail(email), password);
}

/** Com a proteção contra enumeração ligada, o Firebase não diz se o e-mail tem cadastro. */
export async function sendPasswordReset(email: string, locale: Language): Promise<void> {
  emailLanguage(locale);
  try { await sendPasswordResetEmail(auth, normalizeEmail(email)); }
  catch (error) {
    // Sem a proteção (emulador), não revelar quais e-mails têm cadastro: a resposta é a mesma.
    if (!(error instanceof FirebaseError && error.code === "auth/user-not-found")) throw error;
  }
}

export async function resendEmailVerification(locale: Language): Promise<void> {
  if (!auth.currentUser) return;
  emailLanguage(locale);
  await sendEmailVerification(auth.currentUser);
}

/**
 * Cadastro só por senha ainda sem o e-mail confirmado: não entra no app. Quem tem o Google ligado
 * já tem o e-mail comprovado pelo próprio Google. A mesma regra vale no backend e nas Rules.
 */
export function needsEmailVerification(user: User): boolean {
  const providers = user.providerData.map((provider) => provider.providerId);
  return !user.emailVerified && providers.includes("password") && !providers.includes("google.com");
}

/** Relê a conta e, se o e-mail foi confirmado, renova o ID token (o claim `email_verified` muda). */
export async function refreshEmailVerified(): Promise<boolean> {
  const user = auth.currentUser;
  if (!user) return false;
  await user.reload();
  if (needsEmailVerification(user)) return false;
  await user.getIdToken(true);
  return true;
}

/** Texto leigo para os erros do Firebase Authentication; o código técnico fica no console. */
export function describeAuthError(error: unknown): string {
  if (!(error instanceof FirebaseError)) return error instanceof Error ? error.message : String(error);
  const code = error.code;
  console.error("falha de autenticação:", error);
  switch (code) {
    case "auth/invalid-email": return "Esse e-mail não parece válido. Confira e tente de novo.";
    case "auth/email-already-in-use": return "Esse e-mail já tem cadastro. Entre com ele, ou use \"Esqueci minha senha\" para criar uma senha nova.";
    case "auth/weak-password": return "Escolha uma senha mais forte, com pelo menos 8 caracteres.";
    case "auth/invalid-credential":
    case "auth/wrong-password":
    case "auth/user-not-found":
    case "auth/invalid-login-credentials": return "E-mail ou senha incorretos. Se você criou a conta pelo Google, use \"Entrar com Google\" ou \"Esqueci minha senha\".";
    case "auth/user-disabled": return "Esta conta está desativada. Fale com o suporte do ShmupX.";
    case "auth/too-many-requests": return "Muitas tentativas seguidas. Espere alguns minutos e tente de novo.";
    case "auth/network-request-failed": return "Sem conexão com a internet. Confira a sua rede e tente de novo.";
    case "auth/operation-not-allowed": return "O cadastro por e-mail ainda não está liberado. Use \"Entrar com Google\" por enquanto.";
    case "auth/popup-closed-by-user":
    case "auth/cancelled-popup-request": return "login cancelado";
    default: return "Não foi possível entrar agora. Tente de novo em instantes.";
  }
}

/** A conta atual tem o Google ligado (entrou por ele ou o associou ao cadastro por e-mail). */
export function hasGoogle(): boolean { return !!auth.currentUser?.providerData.some((provider) => provider.providerId === "google.com"); }

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
