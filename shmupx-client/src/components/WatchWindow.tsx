import { useEffect, useState } from "react";
import { onAuthStateChanged } from "firebase/auth";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { isTauri } from "@tauri-apps/api/core";
import { auth } from "../auth/firebase";
import { useI18n } from "../i18n";
import { usePlayerPresence } from "../data/useRoomPresence";

/** Parâmetros da janela da live (`index.html?watch=<uid>&room=<gameId>&name=<nick>`). */
export function watchParams(): { uid: string; room: string; name: string } | null {
  const params = new URLSearchParams(window.location.search);
  const uid = params.get("watch");
  const room = params.get("room");
  return uid && room ? { uid, room, name: params.get("name") ?? "" } : null;
}

/** Abre (ou traz para a frente) a janela da live de um jogador da sala. */
export async function openWatchWindow(roomId: string, uid: string, name: string, title: string) {
  const label = `live-${uid.replace(/[^A-Za-z0-9_-]/g, "")}`;
  const query = new URLSearchParams({ watch: uid, room: roomId, name });
  if (!isTauri()) { window.open(`/?${query}`, label, "width=1024,height=640"); return; }
  const existing = await WebviewWindow.getByLabel(label);
  if (existing) { await existing.unminimize().catch(() => {}); await existing.setFocus(); return; }
  new WebviewWindow(label, { url: `index.html?${query}`, title, width: 1024, height: 640, minWidth: 480, minHeight: 300, center: true });
}

function LivePlayer({ room, uid, name }: { room: string; uid: string; name: string }) {
  const { t } = useI18n();
  const presence = usePlayerPresence(room, uid);
  const live = presence?.live;
  const [videoId, setVideoId] = useState<string | null>(null);
  // Mantém o mesmo vídeo enquanto a live segue (o contador muda a cada 30 s sem recarregar o player).
  useEffect(() => { if (live?.videoId) setVideoId(live.videoId); }, [live?.videoId]);
  const ended = presence !== undefined && !live;
  return (
    <main className="watch-window">
      <header className="watch-bar">
        <span className={`presence-dot ${live ? "is-live" : "is-offline"}`} aria-hidden="true" />
        <strong>{presence?.name ?? name}</strong>
        {live && <span className="watch-tag">{t("AO VIVO")}</span>}
        {live && <span className="watch-viewers" role="status">👁 {t("{count} assistindo", { count: live.viewers.toLocaleString() })}</span>}
      </header>
      {presence === undefined && <p className="watch-message">{t("Carregando a live…")}</p>}
      {ended && <p className="watch-message">{t("A live terminou.")}</p>}
      {!ended && videoId && (
        <iframe
          className="watch-player"
          src={`https://www.youtube.com/embed/${videoId}?autoplay=1&playsinline=1`}
          title={t("Live de {name}", { name: presence?.name ?? name })}
          allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
          allowFullScreen
          referrerPolicy="strict-origin-when-cross-origin"
        />
      )}
    </main>
  );
}

/** Janela separada que transmite a live de um jogador da sala, com o contador de quem assiste. */
export default function WatchWindow({ room, uid, name }: { room: string; uid: string; name: string }) {
  const { t } = useI18n();
  // A sessão do Firebase é a mesma da janela principal (mesma origem); espera ela carregar.
  const [signedIn, setSignedIn] = useState<boolean | null>(null);
  useEffect(() => onAuthStateChanged(auth, (user) => setSignedIn(Boolean(user))), []);
  if (signedIn === null) return <main className="watch-window" aria-busy="true" />;
  if (!signedIn) return <main className="watch-window"><p className="watch-message">{t("Entre na sua conta no ShmupX Client para assistir.")}</p></main>;
  return <LivePlayer room={room} uid={uid} name={name} />;
}
