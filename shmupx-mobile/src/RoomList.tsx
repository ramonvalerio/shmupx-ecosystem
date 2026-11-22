import { useState } from "react";
import { useI18n } from "@client/i18n";
import { useGameArt } from "@client/data/gameArt";
import type { GameRoom } from "@client/types/room";
import { useMisterCompatible } from "./MisterPlay";

function RoomCard({ room, onOpen }: { room: GameRoom; onOpen: () => void }) {
  const art = useGameArt(room);
  const mister = useMisterCompatible(room);
  return (
    <button type="button" className="lite-room-card" onClick={onOpen}>
      <img src={art.src} alt="" loading="lazy" onError={art.onError} />
      <span className="lite-room-info">
        <strong>{room.title}</strong>
        {mister && <span className="lite-mister-badge">MiSTer</span>}
        {room.developer && <small>{room.developer}{room.year ? ` · ${room.year}` : ""}</small>}
      </span>
      <span className="lite-chevron" aria-hidden="true">›</span>
    </button>
  );
}

/** Salas (jogos publicados): toque para entrar no chat da sala. */
export default function RoomList({ rooms, profileName, onOpen, onSettings, onSignOut }: { rooms: GameRoom[]; profileName: string; onOpen: (id: string) => void; onSettings: () => void; onSignOut: () => void }) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const search = query.trim().toLocaleLowerCase("pt-BR");
  const visible = rooms.filter((room) => room.title.toLocaleLowerCase("pt-BR").includes(search));
  return (
    <main className="lite-screen">
      <header className="lite-topbar">
        <img src="/img/logo.png" alt="ShmupX" className="lite-topbar-logo" />
        <span className="lite-badge small">LITE</span>
        <button type="button" className="lite-icon-button lite-gear" onClick={onSettings} aria-label={t("Ajustes")} title={t("Ajustes")}>⚙</button>
        <button type="button" className="lite-profile" onClick={onSignOut} title={t("Sair da conta")}>
          <img src="/img/avatar-default.png" alt="" />
          <span>{profileName}</span>
        </button>
      </header>
      <div className="lite-content">
        <h1 className="lite-title">{t("Salas")}</h1>
        <input className="lite-search" type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("Buscar jogo…")} aria-label={t("Buscar jogo")} />
        {rooms.length === 0 && <p className="lite-muted lite-center-text">{t("Carregando as salas…")}</p>}
        <div className="lite-room-list">
          {visible.map((room) => <RoomCard key={room.id} room={room} onOpen={() => onOpen(room.id)} />)}
        </div>
      </div>
    </main>
  );
}
