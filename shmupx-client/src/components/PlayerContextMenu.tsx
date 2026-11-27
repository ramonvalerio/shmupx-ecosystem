import { useEffect, useLayoutEffect, useRef, useState } from "react";

export interface PlayerMenuItem {
  label: string;
  /** Item desligado: o motivo aparece embaixo, em letra menor. */
  disabledReason?: string | null;
  onSelect: () => void;
}

/**
 * Menu do clique direito num jogador da sala (o do sistema fica desligado no Client, ver
 * desktopFeel.ts). Abre onde o mouse está, sem sair da janela, e fecha com Esc, clique fora,
 * rolagem ou a janela perdendo o foco.
 */
export default function PlayerContextMenu({ x, y, title, items, onClose }: { x: number; y: number; title: string; items: PlayerMenuItem[]; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ left: x, top: y });

  useLayoutEffect(() => {
    const menu = ref.current;
    if (!menu) return;
    const { width, height } = menu.getBoundingClientRect();
    setPosition({ left: Math.max(4, Math.min(x, window.innerWidth - width - 4)), top: Math.max(4, Math.min(y, window.innerHeight - height - 4)) });
    menu.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  }, [x, y]);

  useEffect(() => {
    const onDown = (event: MouseEvent) => { if (!ref.current?.contains(event.target as Node)) onClose(); };
    const onKey = (event: KeyboardEvent) => { if (event.key === "Escape") { event.stopPropagation(); onClose(); } };
    window.addEventListener("mousedown", onDown, true);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", onClose);
    window.addEventListener("resize", onClose);
    window.addEventListener("scroll", onClose, true);
    return () => {
      window.removeEventListener("mousedown", onDown, true);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("blur", onClose);
      window.removeEventListener("resize", onClose);
      window.removeEventListener("scroll", onClose, true);
    };
  }, [onClose]);

  return (
    <div ref={ref} className="player-context-menu" role="menu" aria-label={title} style={position} onContextMenu={(event) => event.preventDefault()}>
      <p className="player-context-menu-title">{title}</p>
      {items.map((item) => (
        <button key={item.label} type="button" role="menuitem" disabled={Boolean(item.disabledReason)}
          onClick={() => { onClose(); item.onSelect(); }}>
          <span>{item.label}</span>
          {item.disabledReason && <small>{item.disabledReason}</small>}
        </button>
      ))}
    </div>
  );
}
