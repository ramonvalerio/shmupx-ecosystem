import { useLayoutEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type PointerEvent } from "react";
import { useI18n } from "../i18n";
import type { GameRoom } from "../types/room";
import { moveTab } from "./tabOrder";

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

/** Distância (px) para um clique virar arrasto. */
const DRAG_THRESHOLD = 5;


/**
 * Barra com o botão de volta à coleção e uma aba por sala aberta. As abas se reordenam
 * arrastando (eventos de ponteiro: no Windows o Tauri intercepta o arrastar nativo do HTML) ou com
 * Ctrl+Shift+← / →.
 */
export default function RoomTabs({ rooms, openRoomIds, activeRoomId, recordingRoomId, onShowCollection, onActivate, onClose, onReorder }: RoomTabsProps) {
  const { t } = useI18n();
  const tabRefs = useRef(new Map<string, HTMLDivElement>());
  const drag = useRef<{ id: string; startX: number; grabOffset: number; lastX: number; moved: boolean; pointerId: number } | null>(null);
  const [draggingId, setDraggingId] = useState<string | null>(null);
  // A aba segue o ponteiro (deslocamento a partir do lugar dela) e deixa um rastro no sentido oposto.
  const [offset, setOffset] = useState(0);
  const [direction, setDirection] = useState(1);
  const listRef = useRef<HTMLDivElement>(null);
  // O clique que encerra um arrasto não deve ativar a aba.
  const suppressClick = useRef(false);

  /**
   * Borda esquerda do lugar da aba na fila, em coordenadas da janela. Usa a posição de layout
   * (`offsetLeft`), que ignora o `translateX` do arrasto: medir com `getBoundingClientRect` somaria o
   * deslocamento que a própria aba está aplicando e o erro se acumularia a cada troca de lugar.
   */
  function slotLeft(element: HTMLElement) {
    const list = listRef.current;
    if (!list) return element.getBoundingClientRect().left;
    return list.getBoundingClientRect().left + element.offsetLeft - list.scrollLeft;
  }

  /** Posiciona a aba arrastada sob o ponteiro, a partir do lugar atual dela. */
  function follow(clientX: number) {
    const current = drag.current;
    const element = current && tabRefs.current.get(current.id);
    if (!current || !element) return;
    setOffset(clientX - current.grabOffset - slotLeft(element));
  }

  // Depois de trocar de lugar, o slot da aba mudou: recalcula antes de pintar, sem pulo.
  useLayoutEffect(() => {
    if (drag.current?.moved) follow(drag.current.lastX);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- só quando a ordem muda
  }, [openRoomIds]);

  // Valores atuais para os ouvintes da janela (que vivem durante todo o arrasto).
  const latest = useRef({ openRoomIds, onReorder });
  latest.current = { openRoomIds, onReorder };

  /**
   * Arrasto pela janela inteira: trocar a aba de lugar move o elemento na página, e o navegador
   * solta a captura do ponteiro quando isso acontece; ouvir só a aba perderia o "soltar" e o arrasto
   * ficaria preso até o próximo clique.
   */
  function onPointerDown(event: PointerEvent<HTMLDivElement>, id: string) {
    if (event.button !== 0 || (event.target as HTMLElement).closest(".room-tab-close")) return;
    drag.current = { id, startX: event.clientX, grabOffset: event.clientX - slotLeft(event.currentTarget), lastX: event.clientX, moved: false, pointerId: event.pointerId };

    const move = (e: globalThis.PointerEvent) => {
      const current = drag.current;
      if (!current || e.pointerId !== current.pointerId) return;
      // Botão já solto (o "soltar" se perdeu fora da janela): encerra.
      if (e.buttons === 0) return finish();
      if (!current.moved) {
        if (Math.abs(e.clientX - current.startX) < DRAG_THRESHOLD) return;
        current.moved = true;
        setDraggingId(current.id);
      }
      if (e.clientX !== current.lastX) setDirection(e.clientX > current.lastX ? 1 : -1);
      current.lastX = e.clientX;
      follow(e.clientX);
      // Novo lugar = quantas outras abas têm o centro à esquerda do centro da aba arrastada.
      const ids = latest.current.openRoomIds;
      const dragged = tabRefs.current.get(current.id);
      const draggedCenter = e.clientX - current.grabOffset + (dragged?.offsetWidth ?? 0) / 2;
      const target = ids.filter((other) => {
        if (other === current.id) return false;
        const el = tabRefs.current.get(other);
        return el ? slotLeft(el) + el.offsetWidth / 2 < draggedCenter : false;
      }).length;
      const next = moveTab(ids, current.id, target);
      if (next.some((other, i) => other !== ids[i])) latest.current.onReorder(next);
    };
    const up = (e: globalThis.PointerEvent) => { if (!drag.current || e.pointerId === drag.current.pointerId) finish(); };
    const finish = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      window.removeEventListener("blur", finish);
      const current = drag.current;
      drag.current = null;
      setOffset(0);
      setDraggingId(null);
      // O clique que vem logo depois de um arrasto não ativa a aba; se ele não vier, libera.
      if (current?.moved) {
        suppressClick.current = true;
        window.setTimeout(() => { suppressClick.current = false; }, 0);
      }
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", up);
    window.addEventListener("blur", finish);
  }

  function onTabKeyDown(event: KeyboardEvent<HTMLButtonElement>, id: string) {
    if (!(event.ctrlKey && event.shiftKey) || (event.key !== "ArrowLeft" && event.key !== "ArrowRight")) return;
    event.preventDefault();
    const index = openRoomIds.indexOf(id);
    onReorder(moveTab(openRoomIds, id, index + (event.key === "ArrowLeft" ? -1 : 1)));
  }

  return (
    <div ref={listRef} className="room-tabs" role="tablist" aria-label={t("Salas abertas")}>
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
            ref={(el) => { if (el) tabRefs.current.set(id, el); else tabRefs.current.delete(id); }}
            className={`room-tab ${draggingId === id ? "is-dragging" : ""}`}
            style={draggingId === id ? ({ transform: `translateX(${offset}px)`, "--trail": direction } as CSSProperties) : undefined}
            data-active={active}
            role="presentation"
            onPointerDown={(event) => onPointerDown(event, id)}
          >
            <button
              type="button"
              role="tab"
              aria-selected={active}
              title={`${room.title} · ${t("arraste para reordenar (Ctrl+Shift+← →)")}`}
              onClick={() => { if (suppressClick.current) { suppressClick.current = false; return; } onActivate(id); }}
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
