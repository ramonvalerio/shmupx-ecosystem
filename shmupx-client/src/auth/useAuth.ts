import { useCallback, useEffect, useState } from "react";
import { onAuthStateChanged } from "firebase/auth";
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

  const signIn = useCallback(async () => {
    setState({ status: "signed-out", busy: true });
    try { await signInWithGoogle(); }
    catch (error) { setState({ status: "signed-out", error: describe(error) }); }
  }, []);

  const setProfile = useCallback((profile: PlayerProfile) => setState({ status: "signed-in", profile }), []);

  return { state, signIn, signOut: signOutPlayer, setProfile };
}
