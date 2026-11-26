import { useCallback, useEffect, useState } from "react";
import { doc, onSnapshot } from "firebase/firestore";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { db } from "../auth/firebase";

/** Perfil público do próprio jogador (backend `GET /api/me/profile`). */
export interface MyPublicProfile {
  handle: string;
  /** Já usou a troca livre do nick: outra só pelo suporte. */
  handle_locked: boolean;
  handle_request: { status: string; requested: string } | null;
  avatar: string | null;
}

export const DEFAULT_AVATAR = "/img/avatar-default.png";
const EVENT = "shmupx:my-public-profile";

/** O último perfil público lido (nick e avatar), para quem não é tela (o cartão do Caravan). */
let latest: MyPublicProfile | null = null;
if (typeof window !== "undefined") window.addEventListener(EVENT, (event) => { latest = (event as CustomEvent<MyPublicProfile>).detail; });
export function readMyPublicProfile(): MyPublicProfile | null {
  return latest;
}

/** Meu perfil público; `refresh` relê do backend e avisa todas as telas. */
export function useMyPublicProfile(displayName: string) {
  const [profile, setProfile] = useState<MyPublicProfile | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!isTauri()) return;
    try {
      const value = await invoke<MyPublicProfile>("perfil_obter", { nome: displayName });
      window.dispatchEvent(new CustomEvent(EVENT, { detail: value }));
      setError(null);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    }
  }, [displayName]);

  useEffect(() => {
    const onChange = (event: Event) => setProfile((event as CustomEvent<MyPublicProfile>).detail);
    window.addEventListener(EVENT, onChange);
    void refresh();
    return () => window.removeEventListener(EVENT, onChange);
  }, [refresh]);

  /** Depois de trocar nick/avatar: o backend devolve o perfil (ou relemos). */
  const apply = useCallback((value: MyPublicProfile) => window.dispatchEvent(new CustomEvent(EVENT, { detail: value })), []);
  return { profile, error, refresh, apply };
}

/** Avatar e nick de outros jogadores (`publicProfiles/{uid}`), com cache compartilhado. */
interface PublicInfo { avatar?: string; handle?: string }
const cache = new Map<string, PublicInfo>();
const subscriptions = new Map<string, { count: number; stop: () => void }>();
const CHANGED = "shmupx:public-profiles";

function subscribe(uid: string) {
  const current = subscriptions.get(uid);
  if (current) { current.count += 1; return; }
  const stop = onSnapshot(
    doc(db, "publicProfiles", uid),
    (snapshot) => {
      const data = snapshot.data() ?? {};
      cache.set(uid, {
        avatar: typeof data.avatar === "string" && data.avatar.startsWith("data:image/") ? data.avatar : undefined,
        handle: typeof data.handle === "string" ? data.handle : undefined,
      });
      window.dispatchEvent(new Event(CHANGED));
    },
    () => undefined,
  );
  subscriptions.set(uid, { count: 1, stop });
}

function unsubscribe(uid: string) {
  const current = subscriptions.get(uid);
  if (!current) return;
  current.count -= 1;
  if (current.count <= 0) { current.stop(); subscriptions.delete(uid); }
}

/** `uid -> { avatar, handle }` dos jogadores pedidos, atualizado em tempo real. */
export function usePublicProfiles(uids: string[]): Map<string, PublicInfo> {
  const key = [...new Set(uids.filter(Boolean))].sort().join(",");
  const [, setVersion] = useState(0);
  useEffect(() => {
    const ids = key ? key.split(",") : [];
    ids.forEach(subscribe);
    const onChange = () => setVersion((v) => v + 1);
    window.addEventListener(CHANGED, onChange);
    return () => { window.removeEventListener(CHANGED, onChange); ids.forEach(unsubscribe); };
  }, [key]);
  return cache;
}

/** Avatar de um jogador (ou o padrão). */
export function avatarOf(profiles: Map<string, PublicInfo>, uid: string | undefined): string {
  return (uid && profiles.get(uid)?.avatar) || DEFAULT_AVATAR;
}
