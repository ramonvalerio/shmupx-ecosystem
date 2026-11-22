import { useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { isTauri } from "@tauri-apps/api/core";
import { useI18n } from "@client/i18n";
import ChatFeed from "@client/components/ChatFeed";
import ChatInput from "@client/components/ChatInput";
import { useRoomChat } from "@client/data/useRoomChat";
import { leaveRoomPresence, usePlayerPresence, usePresenceHeartbeat, useRoomPresence, type PresenceStatus } from "@client/data/useRoomPresence";
import type { PlayerProfile } from "@client/auth/firebase";
import type { GameRoom } from "@client/types/room";

const STATUS_ORDER: Record<PresenceStatus, number> = { live: 0, online: 1, away: 2, offline: 3 };
const STATUS_LABEL: Record<PresenceStatus, string> = { live: "Ao vivo", online: "Online", away: "Ausente", offline: "Offline" };

function openExternal(url: string) {
  if (isTauri()) void openUrl(url);
  else window.open(url, "_blank", "noopener");
}

/** Live de um jogador da sala, dentro do app, com o contador de quem assiste. */
function LiveViewer({ roomId, uid, name, onClose }: { roomId: string; uid: string; name: string; onClose: () => void }) {
  const { t } = useI18n();
  const presence = usePlayerPresence(roomId, uid);
  const live = presence?.live;
  const [videoId] = useState(() => presence?.live?.videoId ?? null);
  const current = live?.videoId ?? videoId;
  return (
    <div className="lite-live" role="dialog" aria-modal="true" aria-label={t("Live de {name}", { name })}>
      <header className="lite-live-bar">
        <button type="button" className="lite-icon-button" onClick={onClose} aria-label={t("Fechar")}>✕</button>
        <span className={`presence-dot ${live ? "is-live" : "is-offline"}`} aria-hidden="true" />
        <strong>{name}</strong>
        {live && <span className="lite-live-tag">{t("AO VIVO")}</span>}
        {live && <span className="lite-live-viewers" role="status">👁 {t("{count} assistindo", { count: live.viewers.toLocaleString() })}</span>}
      </header>
      {presence !== undefined && !live && <p className="lite-muted lite-center-text">{t("A live terminou.")}</p>}
      {live && current && <>
        <iframe
          className="lite-live-player"
          src={`https://www.youtube.com/embed/${current}?autoplay=1&playsinline=1`}
          title={t("Live de {name}", { name })}
          allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
          allowFullScreen
          referrerPolicy="strict-origin-when-cross-origin"
        />
        <button type="button" className="lite-secondary" onClick={() => openExternal(`https://www.youtube.com/watch?v=${current}`)}>{t("Abrir no YouTube")}</button>
      </>}
    </div>
  );
}

/** Sala no celular: chat e jogadores (com as lives). Sair pela seta deixa a sala. */
export default function Room({ room, profile, onBack }: { room: GameRoom; profile: PlayerProfile; onBack: () => void }) {
  const { t } = useI18n();
  const [tab, setTab] = useState<"chat" | "players">("chat");
  const [watching, setWatching] = useState<{ uid: string; name: string } | null>(null);
  const author = useMemo(() => ({ uid: profile.uid, displayName: profile.displayName }), [profile.uid, profile.displayName]);
  // Só as mensagens desde que entrou: sair pela seta e voltar começa com o chat limpo.
  const [since] = useState(() => Date.now());
  const chat = useRoomChat(room.id, author, since);
  const others = useRoomPresence(room.id, profile.uid);
  // Presença: online com o app em primeiro plano nesta sala; ausente com o app em segundo plano.
  usePresenceHeartbeat(author, [room.id], room.id, null, null);

  const players = [
    { uid: profile.uid, name: profile.displayName, status: "online" as PresenceStatus, live: undefined, you: true },
    ...others.map((o) => ({ uid: o.uid, name: o.name, status: o.status, live: o.live, you: false }))
      .sort((a, b) => STATUS_ORDER[a.status] - STATUS_ORDER[b.status] || a.name.localeCompare(b.name)),
  ];
  const liveCount = others.filter((o) => o.live).length;

  function leave() {
    void leaveRoomPresence(room.id, profile.uid);
    onBack();
  }

  return (
    <main className="lite-screen lite-room">
      <header className="lite-topbar">
        <button type="button" className="lite-icon-button" onClick={leave} aria-label={t("Sair da sala")}>‹</button>
        <h1 className="lite-room-title" title={room.title}>{room.title}</h1>
      </header>
      <div className="lite-tabs" role="tablist">
        <button type="button" role="tab" aria-selected={tab === "chat"} onClick={() => setTab("chat")}>{t("Chat")}</button>
        <button type="button" role="tab" aria-selected={tab === "players"} onClick={() => setTab("players")}>
          {t("Jogadores ({count})", { count: players.length })}
          {liveCount > 0 && <span className="presence-dot is-live" aria-label={t("Ao vivo")} />}
        </button>
      </div>

      {tab === "chat" ? (
        <section className="lite-chat">
          <ChatFeed messages={chat.messages} status={chat.status} error={chat.error} playerId={profile.uid} />
          <ChatInput onSendMessage={chat.send} disabled={chat.status === "error"} error={chat.error} />
        </section>
      ) : (
        <section className="lite-players">
          <ul>
            {players.map((player) => (
              <li key={player.uid}>
                <button
                  type="button"
                  className={`lite-player ${player.live ? "is-live" : ""}`}
                  disabled={!player.live}
                  onClick={() => player.live && setWatching({ uid: player.uid, name: player.name })}
                >
                  <img src="/img/avatar-default.png" alt="" />
                  <span className="lite-player-name">{player.name}{player.you && ` ${t("(você)")}`}</span>
                  {player.live && <span className="lite-live-tag">{t("ASSISTIR")}</span>}
                  <span className={`presence-dot is-${player.status}`} role="img" aria-label={t(STATUS_LABEL[player.status])} />
                </button>
              </li>
            ))}
          </ul>
          <p className="lite-note">{t("Toque em quem está ao vivo para assistir à live.")}</p>
        </section>
      )}

      {watching && <LiveViewer roomId={room.id} uid={watching.uid} name={watching.name} onClose={() => setWatching(null)} />}
    </main>
  );
}
