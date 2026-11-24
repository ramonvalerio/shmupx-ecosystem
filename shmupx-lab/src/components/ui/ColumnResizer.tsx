import { useCallback, useState, type PointerEvent as ReactPointerEvent } from "react";

export interface ResizableColumn {
  key: string;
  /** Largura inicial, em px. */
  width: number;
  /** Menor largura ao arrastar, em px. */
  min?: number;
}

const MIN_PADRAO = 60;

function lerSalvas(storageKey: string): Record<string, number> {
  try {
    const salvo = JSON.parse(window.localStorage.getItem(storageKey) ?? "{}");
    return salvo && typeof salvo === "object" ? salvo : {};
  } catch {
    return {};
  }
}

/**
 * Larguras das colunas de uma tabela, ajustáveis pelo cabeçalho e lembradas neste computador
 * (`localStorage`, por `storageKey`). Sem armazenamento, valem as larguras iniciais.
 */
export function useColumnWidths(storageKey: string, columns: ResizableColumn[]) {
  const [widths, setWidths] = useState<Record<string, number>>(() => {
    const salvas = lerSalvas(storageKey);
    return Object.fromEntries(
      columns.map((c) => {
        const salva = salvas[c.key];
        return [c.key, typeof salva === "number" && salva >= (c.min ?? MIN_PADRAO) ? salva : c.width];
      }),
    );
  });

  const salvar = useCallback(
    (next: Record<string, number>) => {
      try {
        window.localStorage.setItem(storageKey, JSON.stringify(next));
      } catch {
        // Sem armazenamento: a largura vale só até fechar o Lab.
      }
    },
    [storageKey],
  );

  const resize = useCallback(
    (key: string, width: number, persist: boolean) => {
      const column = columns.find((c) => c.key === key);
      if (!column) return;
      const clamped = Math.max(column.min ?? MIN_PADRAO, Math.round(width));
      setWidths((current) => {
        const next = { ...current, [key]: clamped };
        if (persist) salvar(next);
        return next;
      });
    },
    [columns, salvar],
  );

  const reset = useCallback(
    (key: string) => {
      const column = columns.find((c) => c.key === key);
      if (column) resize(key, column.width, true);
    },
    [columns, resize],
  );

  return { widths, resize, reset };
}

/** Alça na borda direita de um `<th relative>`: arrastar muda a largura; duplo clique volta à inicial. */
export function ColumnResizer({
  label,
  width,
  onResize,
  onReset,
}: {
  /** Nome da coluna, para leitores de tela. */
  label: string;
  width: number;
  onResize: (width: number, persist: boolean) => void;
  onReset: () => void;
}) {
  function onPointerDown(event: ReactPointerEvent<HTMLDivElement>) {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    const handle = event.currentTarget;
    const startX = event.clientX;
    const startWidth = width;
    let last = startWidth;
    handle.setPointerCapture(event.pointerId);
    document.body.style.cursor = "col-resize";

    const move = (e: PointerEvent) => {
      last = startWidth + e.clientX - startX;
      onResize(last, false);
    };
    const up = (e: PointerEvent) => {
      handle.releasePointerCapture(e.pointerId);
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", up);
      handle.removeEventListener("pointercancel", up);
      document.body.style.cursor = "";
      onResize(last, true);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", up);
    handle.addEventListener("pointercancel", up);
  }

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={`Largura da coluna ${label}`}
      title="Arraste para ajustar a largura · duplo clique volta ao padrão"
      onPointerDown={onPointerDown}
      onDoubleClick={onReset}
      onClick={(event) => event.stopPropagation()}
      className="group absolute top-0 -right-1.5 z-10 flex h-full w-3 cursor-col-resize touch-none justify-center"
    >
      <span className="my-2 w-px bg-white/10 transition-colors group-hover:bg-cyan-400/70 group-active:bg-cyan-400" />
    </div>
  );
}
