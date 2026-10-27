import { useI18n } from "../i18n";
import type { GameRoom } from "../types/room";

interface RoomTabsProps {
  rooms: GameRoom[];
  /** Salas abertas, na ordem das abas. */
  openRoomIds: string[];
  /** Aba ativa; `null` = a grade da coleção. */
  activeRoomId: string | null;
  recordingRoomId: string | null;
  onShowCollection: () => void;
  onActivate: (id: string) => void;
  onClose: (id: string) => void;
}

/** Barra com o botão de volta à coleção e uma aba por sala aberta. */
export default function RoomTabs({ rooms, openRoomIds, activeRoomId, recordingRoomId, onShowCollection, onActivate, onClose }: RoomTabsProps) {
  const { t } = useI18n();
  return (
    <div className="room-tabs" role="tablist" aria-label={t("Salas abertas")}>
      <button type="button" role="tab" className="room-tabs-home" aria-selected={activeRoomId === null} onClick={onShowCollection}>
        {t("← Coleção")}
      </button>
      {openRoomIds.map((id) => {
        const room = rooms.find((candidate) => candidate.id === id);
        if (!room) return null;
        const active = activeRoomId === id;
        return (
          <div key={id} className="room-tab" data-active={active} role="presentation">
            <button type="button" role="tab" aria-selected={active} title={room.title} onClick={() => onActivate(id)}>
              {recordingRoomId === id && <span className="room-tab-recording" aria-label={t("Gravando")} />}
              {room.title}
            </button>
            <button type="button" className="room-tab-close" aria-label={t("Fechar sala de {game}", { game: room.title })} onClick={() => onClose(id)}>×</button>
          </div>
        );
      })}
    </div>
  );
}
