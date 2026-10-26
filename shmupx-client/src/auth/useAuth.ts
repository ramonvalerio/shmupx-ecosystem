import { useCallback, useEffect, useState } from "react";
import { onAuthStateChanged, onIdTokenChanged } from "firebase/auth";
import { isTauri, invoke } from "@tauri-apps/api/core";
import { auth, ensurePlayerProfile, signInWithGoogle, signOutPlayer, type PlayerProfile } from "./firebase";
import { useI18n } from "../i18n";

export type AuthState =
  | { status: "loading" }
  | { status: "signed-out"; error?: string; busy?: boolean }
  | { status: "signed-in"; profile: PlayerProfile };

function describe(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useAuth() {
  const { language } = useI18n();
  const [state, setState] = useState<AuthState>({ status: "loading" });

  useEffect(() => onAuthStateChanged(auth, (user) => {
    if (!user) { setState((current) => current.status === "signed-out" ? current : { status: "signed-out" }); return; }
    ensurePlayerProfile(user, language)
      .then((profile) => setState({ status: "signed-in", profile }))
      .catch((error) => {
        // Sem perfil não há sessão utilizável: desfaz o login em vez de deixar meio conectado.
        void signOutPlayer();
        setState({ status: "signed-out", error: `Não foi possível carregar o perfil: ${describe(error)}` });
      });
  // O idioma só vale para o perfil criado na primeira entrada; não reassinar por causa dele.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }), []);

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
    setState({ status: "signed-out", busy: true });
    try { await signInWithGoogle(); }
    catch (error) { setState({ status: "signed-out", error: describe(error) }); }
  }, []);

  const setProfile = useCallback((profile: PlayerProfile) => setState({ status: "signed-in", profile }), []);

  return { state, signIn, signOut: signOutPlayer, setProfile };
}
