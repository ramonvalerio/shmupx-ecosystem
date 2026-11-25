import { useCallback, useEffect, useRef, useState } from "react";
import { onAuthStateChanged, onIdTokenChanged, type User } from "firebase/auth";
import { isTauri, invoke } from "@tauri-apps/api/core";
import {
  auth, describeAuthError, ensurePlayerProfile, linkGoogleWithPassword, needsEmailVerification, PendingGoogleLink,
  refreshEmailVerified, resendEmailVerification, sendPasswordReset, signInWithEmail, signInWithGoogle, signOutPlayer,
  signUpWithEmail, type PlayerProfile,
} from "./firebase";
import { useI18n } from "../i18n";

export type AuthState =
  | { status: "loading" }
  /** `linkEmail`: o Google entrou com um e-mail que já tem cadastro por senha (ver PendingGoogleLink). */
  | { status: "signed-out"; error?: string; busy?: boolean; linkEmail?: string }
  /** Cadastro por senha esperando o jogador abrir o link de confirmação enviado ao e-mail. */
  | { status: "verify-email"; email: string }
  | { status: "signed-in"; profile: PlayerProfile };

export function useAuth() {
  const { language } = useI18n();
  const [state, setState] = useState<AuthState>({ status: "loading" });
  const pendingLink = useRef<PendingGoogleLink | null>(null);
  const checking = useRef<Promise<boolean> | null>(null);
  const languageRef = useRef(language);
  languageRef.current = language;

  const loadProfile = useCallback((user: User) => {
    ensurePlayerProfile(user, languageRef.current)
      .then((profile) => setState({ status: "signed-in", profile }))
      .catch((error) => {
        // Sem perfil não há sessão utilizável: desfaz o login em vez de deixar meio conectado.
        console.error("falha ao carregar o perfil:", error);
        void signOutPlayer();
        setState({ status: "signed-out", error: "Não foi possível carregar o seu perfil. Confira a internet e entre de novo." });
      });
  }, []);

  useEffect(() => onAuthStateChanged(auth, (user) => {
    checking.current = null;
    if (!user) { setState((current) => current.status === "signed-out" ? current : { status: "signed-out" }); return; }
    // O perfil só nasce depois da confirmação: é quando o nome do cadastro já está gravado na conta.
    if (needsEmailVerification(user)) { setState({ status: "verify-email", email: user.email ?? "" }); return; }
    loadProfile(user);
  }), [loadProfile]);

  // O Rust chama o backend sozinho (timer seguro expirando, push-to-talk), então precisa sempre
  // do ID token vigente. onIdTokenChanged dispara no login, na renovação horária e no logout.
  useEffect(() => {
    if (!isTauri()) return;
    return onIdTokenChanged(auth, (user) => {
      const sync = user
        ? user.getIdToken().then((token) => invoke("definir_token_autenticacao", { token }))
        : invoke("limpar_token_autenticacao");
      sync.catch((error) => console.error("falha ao sincronizar a sessão com o Rust:", error));
    });
  }, []);

  const signIn = useCallback(async () => {
    pendingLink.current = null;
    setState({ status: "signed-out", busy: true });
    try { await signInWithGoogle(); }
    catch (error) {
      if (error instanceof PendingGoogleLink) {
        pendingLink.current = error;
        setState({ status: "signed-out", linkEmail: error.email });
        return;
      }
      setState({ status: "signed-out", error: describeAuthError(error) });
    }
  }, []);

  /** Desiste de esperar o navegador (o jogador fechou a aba ou quer tentar de novo). */
  const cancelSignIn = useCallback(() => { pendingLink.current = null; setState({ status: "signed-out" }); }, []);

  // As ações de e-mail devolvem a mensagem leiga do erro (ou nada, se deram certo): a tela decide
  // onde mostrá-la, sem tirar o jogador do formulário que ele estava preenchendo.
  const run = useCallback(async (action: () => Promise<void>): Promise<string | null> => {
    try { await action(); return null; }
    catch (error) { return describeAuthError(error); }
  }, []);

  const signInEmail = useCallback((email: string, password: string) => run(() => signInWithEmail(email, password)), [run]);
  const signUpEmail = useCallback((name: string, email: string, password: string) => run(() => signUpWithEmail(name, email, password, languageRef.current)), [run]);
  const resetPassword = useCallback((email: string) => run(() => sendPasswordReset(email, languageRef.current)), [run]);
  const resendVerification = useCallback(() => run(() => resendEmailVerification(languageRef.current)), [run]);

  const linkGoogle = useCallback((password: string) => run(async () => {
    const pending = pendingLink.current;
    if (!pending) throw new Error("Entre com o Google de novo para ligar a conta.");
    await linkGoogleWithPassword(pending.email, password, pending.credential);
    pendingLink.current = null;
  }), [run]);

  /**
   * `true` quando o e-mail já foi confirmado e o app seguiu para o perfil. A tela confere sozinha
   * (a cada 5 s e ao voltar o foco) e pelo botão: uma conferência por vez, e o perfil carrega uma vez.
   */
  const checkVerified = useCallback((): Promise<boolean> => {
    checking.current ??= (async () => {
      try {
        if (!(await refreshEmailVerified()) || !auth.currentUser) { checking.current = null; return false; }
        // Confirmado: fica respondendo `true` sem recarregar o perfil até a conta mudar.
        loadProfile(auth.currentUser);
        return true;
      } catch (error) {
        console.error("falha ao conferir a confirmação do e-mail:", error);
        checking.current = null;
        return false;
      }
    })();
    return checking.current;
  }, [loadProfile]);

  const setProfile = useCallback((profile: PlayerProfile) => setState({ status: "signed-in", profile }), []);

  return {
    state, signIn, cancelSignIn, signOut: signOutPlayer, setProfile,
    signInEmail, signUpEmail, resetPassword, resendVerification, linkGoogle, checkVerified,
  };
}

export type Auth = ReturnType<typeof useAuth>;
