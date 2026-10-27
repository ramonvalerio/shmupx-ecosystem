import { useCallback, useEffect, useState } from "react";
import { onAuthStateChanged } from "firebase/auth";
import { auth, signInWithGoogle, signOutAdmin } from "./firebase";

export type AuthState =
  | { status: "loading" }
  | { status: "signed-out"; error?: string; busy?: boolean }
  | { status: "signed-in"; email: string };

function describe(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useAuth() {
  const [state, setState] = useState<AuthState>({ status: "loading" });

  useEffect(
    () =>
      onAuthStateChanged(auth, (user) =>
        setState(user ? { status: "signed-in", email: user.email ?? user.uid } : { status: "signed-out" }),
      ),
    [],
  );

  const signIn = useCallback(async () => {
    setState({ status: "signed-out", busy: true });
    try {
      await signInWithGoogle();
    } catch (error) {
      setState({ status: "signed-out", error: describe(error) });
    }
  }, []);

  return { state, signIn, signOut: signOutAdmin };
}
