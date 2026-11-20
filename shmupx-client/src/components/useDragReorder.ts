import { useLayoutEffect, useRef, useState, type CSSProperties, type PointerEvent as ReactPointerEvent } from "react";
import { moveTab } from "./tabOrder";

/** Distância (px) para um clique virar arrasto. */
const DRAG_THRESHOLD = 5;

interface Options {
  ids: string[];
  onReorder: (ids: string[]) => void;
  /** Eixo do arrasto: abas (x) ou menu lateral (y). */
  axis: "x" | "y";
}

/**
 * Reordenar arrastando, usado pelas abas das salas e pelo menu lateral. O item segue o ponteiro e
 * troca de lugar quando o centro dele passa o centro do vizinho.
 *
 * - Ouve o ponteiro na janela inteira: trocar o item de lugar move o elemento na página, e o
 *   navegador solta a captura do ponteiro nessa hora; ouvir só o item perderia o "soltar".
 * - Mede pela posição de layout (`offsetLeft`/`offsetTop`), que ignora o deslocamento do arrasto.
 */
export function useDragReorder({ ids, onReorder, axis }: Options) {
  const items = useRef(new Map<string, HTMLElement>());
  const listRef = useRef<HTMLDivElement>(null);
  const drag = useRef<{ id: string; start: number; grab: number; last: number; moved: boolean; pointerId: number } | null>(null);
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [offset, setOffset] = useState(0);
  const [direction, setDirection] = useState(1);
  const suppressClick = useRef(false);
  const latest = useRef({ ids, onReorder });
  latest.current = { ids, onReorder };

  const coord = (e: { clientX: number; clientY: number }) => (axis === "x" ? e.clientX : e.clientY);
  const size = (el: HTMLElement) => (axis === "x" ? el.offsetWidth : el.offsetHeight);
  /** Início do lugar do item na fila, em coordenadas da janela. */
  function slotStart(el: HTMLElement) {
    const list = listRef.current;
    if (!list) return axis === "x" ? el.getBoundingClientRect().left : el.getBoundingClientRect().top;
    const rect = list.getBoundingClientRect();
    return axis === "x" ? rect.left + el.offsetLeft - list.scrollLeft : rect.top + el.offsetTop - list.scrollTop;
  }
  function follow(pos: number) {
    const current = drag.current;
    const el = current && items.current.get(current.id);
    if (current && el) setOffset(pos - current.grab - slotStart(el));
  }

  // Depois de trocar de lugar, o slot mudou: recalcula antes de pintar, sem pulo.
  useLayoutEffect(() => {
    if (drag.current?.moved) follow(drag.current.last);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- só quando a ordem muda
  }, [ids]);

  function onPointerDown(event: ReactPointerEvent<HTMLElement>, id: string, ignore?: string) {
    if (event.button !== 0 || (ignore && (event.target as HTMLElement).closest(ignore))) return;
    const pos = coord(event);
    drag.current = { id, start: pos, grab: pos - slotStart(event.currentTarget), last: pos, moved: false, pointerId: event.pointerId };

    const move = (e: PointerEvent) => {
      const current = drag.current;
      if (!current || e.pointerId !== current.pointerId) return;
      if (e.buttons === 0) return finish();
      const p = coord(e);
      if (!current.moved) {
        if (Math.abs(p - current.start) < DRAG_THRESHOLD) return;
        current.moved = true;
        setDraggingId(current.id);
      }
      if (p !== current.last) setDirection(p > current.last ? 1 : -1);
      current.last = p;
      follow(p);
      const list = latest.current.ids;
      const dragged = items.current.get(current.id);
      const center = p - current.grab + (dragged ? size(dragged) : 0) / 2;
      const target = list.filter((other) => {
        if (other === current.id) return false;
        const el = items.current.get(other);
        return el ? slotStart(el) + size(el) / 2 < center : false;
      }).length;
      const next = moveTab(list, current.id, target);
      if (next.some((other, i) => other !== list[i])) latest.current.onReorder(next);
    };
    const up = (e: PointerEvent) => { if (!drag.current || e.pointerId === drag.current.pointerId) finish(); };
    const finish = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", up);
      window.removeEventListener("blur", finish);
      const current = drag.current;
      drag.current = null;
      setOffset(0);
      setDraggingId(null);
      // O clique que vem logo depois de um arrasto não ativa o item; se ele não vier, libera.
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

  return {
    listRef,
    draggingId,
    /** Ref de cada item (para medir o lugar dele). */
    register: (id: string) => (el: HTMLElement | null) => { if (el) items.current.set(id, el); else items.current.delete(id); },
    onPointerDown,
    /** Deslocamento do item arrastado e o sentido do rastro (`--trail`). `extra` vai depois (ex.: rotate). */
    styleFor: (id: string, extra = ""): CSSProperties | undefined =>
      draggingId === id
        ? ({ transform: `${axis === "x" ? "translateX" : "translateY"}(${offset}px) ${extra}`.trim(), "--trail": direction } as CSSProperties)
        : undefined,
    /** true (e consome) se o clique é o fim de um arrasto. */
    consumeClick: () => { if (!suppressClick.current) return false; suppressClick.current = false; return true; },
  };
}
