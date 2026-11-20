import type { KeyboardEvent } from "react";
import { useI18n } from "../i18n";
import type { GameRoom } from "../types/room";
import { moveTab } from "./tabOrder";
import { useDragReorder } from "./useDragReorder";

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
  /** Nova ordem das abas (arrastar ou Ctrl+Shift+setas). */
  onReorder: (ids: string[]) => void;
}

/**
 * Barra com o botão de volta à coleção e uma aba por sala aberta. As abas se reordenam
 * arrastando (ver `useDragReorder`) ou com Ctrl+Shift+← / →.
 */
export default function RoomTabs({ rooms, openRoomIds, activeRoomId, recordingRoomId, onShowCollection, onActivate, onClose, onReorder }: RoomTabsProps) {
  const { t } = useI18n();
  const drag = useDragReorder({ ids: openRoomIds, onReorder, axis: "x" });

  function onTabKeyDown(event: KeyboardEvent<HTMLButtonElement>, id: string) {
    if (!(event.ctrlKey && event.shiftKey) || (event.key !== "ArrowLeft" && event.key !== "ArrowRight")) return;
    event.preventDefault();
    const index = openRoomIds.indexOf(id);
    onReorder(moveTab(openRoomIds, id, index + (event.key === "ArrowLeft" ? -1 : 1)));
  }

  return (
    <div ref={drag.listRef} className="room-tabs" role="tablist" aria-label={t("Salas abertas")}>
      <button type="button" role="tab" className="room-tabs-home" aria-selected={activeRoomId === null} onClick={onShowCollection}>
        {t("← Coleção")}
      </button>
      {openRoomIds.map((id) => {
        const room = rooms.find((candidate) => candidate.id === id);
        if (!room) return null;
        const active = activeRoomId === id;
        return (
          <div
            key={id}
            ref={drag.register(id)}
            className={`room-tab ${drag.draggingId === id ? "is-dragging" : ""}`}
            style={drag.styleFor(id)}
            data-active={active}
            role="presentation"
            onPointerDown={(event) => drag.onPointerDown(event, id, ".room-tab-close")}
          >
            <button
              type="button"
              role="tab"
              aria-selected={active}
              title={`${room.title} · ${t("arraste para reordenar (Ctrl+Shift+← →)")}`}
              onClick={() => { if (!drag.consumeClick()) onActivate(id); }}
              onKeyDown={(event) => onTabKeyDown(event, id)}
            >
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
