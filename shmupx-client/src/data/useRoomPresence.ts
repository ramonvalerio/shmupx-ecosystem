import { useEffect, useState } from "react";
import { collection, deleteDoc, doc, onSnapshot, serverTimestamp, setDoc, type Timestamp } from "firebase/firestore";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { db } from "../auth/firebase";

/** De quanto em quanto tempo o Client confirma que continua conectado. */
const HEARTBEAT_MS = 30_000;
/** Sem confirmação há mais que isso (app fechado, sem rede), o jogador aparece offline. */
const STALE_MS = 75_000;
/** Reavalia quem ficou sem confirmar, mesmo sem mudança no Firestore. */
const TICK_MS = 15_000;

/** Verde: na sala, com o Client em primeiro plano. Vermelho: Client aberto, mas em outra tela ou
 * janela. Vermelho pulsando: transmitindo ao vivo (ausente da sala, jogando para a live). Cinza:
 * Client fechado ou sem conexão (continua na sala até sair pela aba). */
export type PresenceStatus = "online" | "away" | "live" | "offline";

export interface RoomPresence {
  uid: string;
  name: string;
  status: PresenceStatus;
  playing: boolean;
  /** Live no ar: vídeo do YouTube e quantos assistem (atualizado a cada 30 s por quem transmite). */
  live?: { videoId: string; viewers: number };
  /** Jogo aberto na conta do jogador (em qualquer sala, modo ou plataforma). */
  game?: { id: string; title: string; kind?: PlayingKind };
}

/** Modo do jogo aberto: Training (sem ficha), Ranked (vale ranking) ou Live (Ranked ao vivo). */
export type PlayingKind = "training" | "ranked" | "live";
const PLAYING_KINDS: readonly string[] = ["training", "ranked", "live"];

/** Jogo aberto na conta, publicado na presença de todas as salas abertas. */
export interface PresenceGame {
  gameId: string;
  title: string;
  kind?: PlayingKind;
}

/** Live do jogador nesta sala (o vídeo do YouTube é o id da transmissão). */
export interface MyLive {
  roomId: string;
  videoId: string;
}

/** Id do vídeo no link da live (`https://www.youtube.com/watch?v=<id>`). */
export function videoIdFromUrl(url: string | null | undefined): string | null {
  if (!url) return null;
  try {
    const id = new URL(url).searchParams.get("v");
    return id && /^[A-Za-z0-9_-]{6,64}$/.test(id) ? id : null;
  } catch {
    return null;
  }
}

function presenceFrom(uid: string, data: Record<string, unknown>, now: number): RoomPresence {
  const lastSeen = (data.lastSeen as Timestamp | null)?.toMillis() ?? Date.now();
  const offline = now - lastSeen >= STALE_MS;
  const state = typeof data.state === "string" ? data.state : "online";
  const videoId = typeof data.liveVideoId === "string" ? data.liveVideoId : null;
  const live = !offline && state === "live" && videoId ? { videoId, viewers: typeof data.liveViewers === "number" ? data.liveViewers : 0 } : undefined;
  const status: PresenceStatus = offline ? "offline" : live ? "live" : state === "away" ? "away" : "online";
  const game = !offline && typeof data.playingGameId === "string" && typeof data.playingTitle === "string"
    ? { id: data.playingGameId, title: data.playingTitle, kind: typeof data.playingKind === "string" && PLAYING_KINDS.includes(data.playingKind) ? data.playingKind as PlayingKind : undefined } : undefined;
  return { uid, name: typeof data.name === "string" ? data.name : "Jogador", status, playing: !offline && (state === "playing" || Boolean(game)), live, game };
}

function presenceRef(roomId: string, uid: string) {
  return doc(db, "rooms", roomId, "presence", uid);
}

/** Sair da sala pela aba: o jogador deixa a lista. Fechar o app ou cair a conexão não tira. */
export function leaveRoomPresence(roomId: string, uid: string) {
  return deleteDoc(presenceRef(roomId, uid)).catch((failure) => console.warn("falha ao sair da sala:", failure));
}

/** Client em primeiro plano (janela com foco e visível). */
function useForeground() {
  const [foreground, setForeground] = useState(() => document.visibilityState === "visible" && document.hasFocus());
  useEffect(() => {
    const update = () => setForeground(document.visibilityState === "visible" && document.hasFocus());
    window.addEventListener("focus", update);
    window.addEventListener("blur", update);
    document.addEventListener("visibilitychange", update);
    return () => {
      window.removeEventListener("focus", update);
      window.removeEventListener("blur", update);
      document.removeEventListener("visibilitychange", update);
    };
  }, []);
  return foreground;
}

/**
 * Presença do jogador em todas as salas abertas nas abas: `rooms/{gameId}/presence/{uid}` no
 * Firestore (Security Rules: só o próprio jogador grava o seu, com o nome do perfil e o horário do
 * servidor). Confirma a cada 30 s e quando o estado muda: jogando, na sala em primeiro plano
 * (online) ou com o Client aberto em outro lugar (away). Com um jogo aberto na conta (`playing`),
 * todas as salas mostram qual é.
 */
export function usePresenceHeartbeat(me: { uid: string; displayName: string }, openRoomIds: string[], visibleRoomId: string | null, recordingRoomId: string | null, live: MyLive | null, away = false, playing: PresenceGame | null = null) {
  const foreground = useForeground();
  const rooms = openRoomIds.join("\n");
  const liveRoom = live?.roomId ?? null;
  const liveVideo = live?.videoId ?? null;
  const playingId = playing?.gameId ?? null;
  const playingTitle = playing ? playing.title.trim().slice(0, 80) : null;
  const playingKind = playing?.kind ?? null;
  useEffect(() => {
    if (!me.uid || !rooms) return;
    const ids = rooms.split("\n");
    let viewers = 0;
    const beat = async () => {
      // Quem transmite publica quantos assistem (a janela da live dos outros mostra o número).
      if (liveVideo && isTauri()) viewers = await invoke<number>("live_espectadores", { broadcastId: liveVideo }).catch(() => viewers);
      for (const roomId of ids) {
        const base = { name: me.displayName, lastSeen: serverTimestamp() };
        const data = liveVideo && liveRoom === roomId
          ? { ...base, state: "live", liveVideoId: liveVideo, liveViewers: viewers }
          : { ...base, state: recordingRoomId === roomId || playingId === roomId ? "playing" : !away && foreground && visibleRoomId === roomId ? "online" : "away" };
        const ref = presenceRef(roomId, me.uid);
        if (playingId && playingTitle) {
          // Regras antigas (sem os campos do jogo) recusam a gravação: tenta de novo sem o jogo.
          setDoc(ref, { ...data, playingGameId: playingId, playingTitle, ...(playingKind ? { playingKind } : {}) })
            // Regras antigas: sem o modo, depois sem o jogo.
            .catch(() => setDoc(ref, { ...data, playingGameId: playingId, playingTitle }))
            .catch(() => setDoc(ref, data))
            .catch((failure) => console.warn("falha ao registrar a presença na sala:", failure));
        } else {
          setDoc(ref, data).catch((failure) => console.warn("falha ao registrar a presença na sala:", failure));
        }
      }
    };
    void beat();
    const timer = window.setInterval(() => void beat(), HEARTBEAT_MS);
    return () => window.clearInterval(timer);
  }, [me.uid, me.displayName, rooms, visibleRoomId, recordingRoomId, foreground, liveRoom, liveVideo, away, playingId, playingTitle, playingKind]);
}

/** Presença de um jogador (a janela da live acompanha quem transmite). */
export function usePlayerPresence(roomId: string, uid: string): RoomPresence | null | undefined {
  const [data, setData] = useState<Record<string, unknown> | null | undefined>(undefined);
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => onSnapshot(
    presenceRef(roomId, uid),
    (snapshot) => setData(snapshot.exists() ? snapshot.data({ serverTimestamps: "estimate" }) : null),
    (failure) => { console.warn("falha ao ler a presença do jogador:", failure); setData(null); },
  ), [roomId, uid]);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), TICK_MS);
    return () => window.clearInterval(timer);
  }, []);
  return data === undefined ? undefined : data === null ? null : presenceFrom(uid, data, now);
}

/** Os outros jogadores da sala (quem entrou e não saiu pela aba), com o status de cada um. */
export function useRoomPresence(roomId: string, myUid: string): RoomPresence[] {
  const [entries, setEntries] = useState<Array<{ uid: string; data: Record<string, unknown> }>>([]);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    setEntries([]);
    return onSnapshot(
      collection(db, "rooms", roomId, "presence"),
      (snapshot) =>
        setEntries(
          snapshot.docs.map((d) => ({ uid: d.id, data: d.data({ serverTimestamps: "estimate" }) })),
        ),
      (failure) => console.warn("falha ao ler a presença da sala:", failure),
    );
  }, [roomId]);

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), TICK_MS);
    return () => window.clearInterval(timer);
  }, []);

  // O próprio jogador a tela mostra sempre, pelo estado local.
  return entries.filter((e) => e.uid !== myUid).map(({ uid, data }) => presenceFrom(uid, data, now));
}
