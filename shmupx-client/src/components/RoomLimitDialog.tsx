import type { Ref } from "react";
import { useI18n } from "../i18n";
import type { GameRoom } from "../types/room";
import { useGameArt } from "../data/gameArt";
import { useRoomPresence } from "../data/useRoomPresence";
import DialogClose from "./DialogClose";

/**
 * Limite de salas abertas: ao abrir mais uma, o jogador escolhe de qual sair. Cada sala numa linha,
 * com a capa do jogo, o número da sala (a ordem da barra lateral), quantos jogadores estão nela e o
 * "Sair e trocar". A sala com uma run em andamento não sai. Fecha no X, no Esc ou clicando fora.
 */
export default function RoomLimitDialog({ dialogRef, max, rooms, entering, recordingRoomId, myUid, onPick, onClose }: {
  dialogRef: Ref<HTMLDivElement>;
  max: number;
  /** As salas abertas, na ordem da barra lateral. */
  rooms: GameRoom[];
  /** O jogo que o jogador quer abrir. */
  entering: GameRoom;
  recordingRoomId: string | null;
  myUid: string;
  onPick: (roomId: string) => void;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [before, after] = t("Escolha de qual sala sair para entrar em {game} no lugar dela.", { game: "\u0000" }).split("\u0000");
  return (
    <div className="controller-dialog-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby="room-limit-title" tabIndex={-1} className="controller-dialog room-limit-dialog">
        <header>
          <div>
            <h2 id="room-limit-title">{t("Até {count} salas abertas ao mesmo tempo", { count: max })}</h2>
            <p>{before}<strong className="room-limit-game">{entering.title}</strong>{after}</p>
          </div>
          <DialogClose onClick={onClose} />
        </header>
        <ul className="room-limit-list">
          {rooms.map((room, index) => (
            <RoomLimitRow key={room.id} room={room} number={index + 1} recording={recordingRoomId === room.id} myUid={myUid} onPick={() => onPick(room.id)} />
          ))}
        </ul>
      </div>
    </div>
  );
}

function RoomLimitRow({ room, number, recording, myUid, onPick }: { room: GameRoom; number: number; recording: boolean; myUid: string; onPick: () => void }) {
  const { t } = useI18n();
  const art = useGameArt(room);
  // Os outros jogadores na sala, mais o próprio jogador (que está nela).
  const players = useRoomPresence(room.id, myUid).filter((p) => p.status !== "offline").length + 1;
  return (
    <li className={recording ? "is-recording" : undefined}>
      <img className={`room-limit-art ${art.grayscale ? "is-not-owned" : ""}`} src={art.src} alt="" onError={art.onError} />
      <div className="room-limit-info">
        <strong>{room.title}</strong>
        <small>
          {t("Sala {number}", { number })} · <span>{players}</span> {t(players === 1 ? "jogador" : "jogadores")}
        </small>
      </div>
      <button type="button" className="room-limit-swap" disabled={recording} title={recording ? t("Há uma run em andamento nesta sala.") : undefined} onClick={onPick}>
        {recording ? t("Em partida") : <>
          <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M14 4h4a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2h-4" /><path d="M10 16l-4-4 4-4" /><path d="M6 12h10" />
          </svg>
          {t("Sair e trocar")}
        </>}
      </button>
    </li>
  );
}
