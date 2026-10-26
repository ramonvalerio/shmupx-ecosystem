import { useI18n } from "../i18n";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { DASHBOARD_KEY, defaultDashboard, reorderPanel, validateDashboard, type DashboardPanel, type PanelId } from "./dashboardState";

const PANEL_NAMES: Record<PanelId, string> = { performance: "Desempenho por modo", games: "Seus jogos", community: "Jogos da comunidade", ranking: "Ranking", events: "Eventos" };
function readLayout(): DashboardPanel[] {
  try { return validateDashboard(JSON.parse(localStorage.getItem(DASHBOARD_KEY) ?? "null")) ?? defaultDashboard(); }
  catch { return defaultDashboard(); }
}

export default function DashboardWorkspace({ customizing, children }: { customizing: boolean; children: Record<string, ReactNode> }) {
  const { t } = useI18n();
  const [panels, setPanels] = useState(readLayout);
  const [storageError, setStorageError] = useState(false);
  const [notice, setNotice] = useState("");
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const dragging = useRef<{ id: PanelId; x: number; y: number; moved: boolean; target: string; pointerId: number; handle: HTMLButtonElement } | null>(null);
  const resize = useRef<{ id: PanelId; y: number; original: DashboardPanel[]; pointerId: number; handle: HTMLButtonElement } | null>(null);
  useEffect(() => {
    try { localStorage.setItem(DASHBOARD_KEY, JSON.stringify(panels)); setStorageError(false); }
    catch { setStorageError(true); }
  }, [panels]);
  function move(id: PanelId, target: number) {
    setPanels((current) => reorderPanel(current, id, target));
    setNotice(`${PANEL_NAMES[id]} reposicionado.`);
  }
  function setHeight(id: PanelId, height: number) {
    setPanels((current) => current.map((panel) => panel.id === id ? { ...panel, height: Math.max(240, Math.min(800, Math.round(height))) } : panel));
  }
  function cancelResize() {
    const pending = resize.current;
    if (!pending) return;
    resize.current = null;
    setPanels(pending.original);
    if (pending.handle.hasPointerCapture(pending.pointerId)) pending.handle.releasePointerCapture(pending.pointerId);
    setNotice("Redimensionamento cancelado.");
  }
  function cancelDrag() {
    const pending = dragging.current;
    dragging.current = null;
    setDropTarget(null);
    if (pending?.handle.hasPointerCapture(pending.pointerId)) pending.handle.releasePointerCapture(pending.pointerId);
  }
  return <>
    {customizing && <div className="workspace-instructions"><p>{t("Arraste pela alça para mover. Use o canto inferior para ajustar a altura ou os botões para ajustar pelo teclado. Esc cancela o redimensionamento.")}</p><button className="secondary-action" onClick={() => { cancelResize(); setPanels(defaultDashboard()); setNotice("Layout original restaurado."); }}>{t("Restaurar layout")}</button></div>}
    <div className="dashboard-grid" onKeyDown={(event) => { if (event.key === "Escape" && (resize.current || dragging.current)) { event.preventDefault(); event.stopPropagation(); cancelResize(); cancelDrag(); } }}>
      {panels.map((panel, index) => <div key={panel.id} data-dashboard-panel={panel.id} className={`dashboard-tile ${panel.wide ? "wide-panel" : ""} ${customizing ? "is-customizing" : ""} ${dropTarget === panel.id ? "is-drop-target" : ""}`} style={{ height: panel.height }}>
        {customizing && <div className="panel-tools">
          <button className="panel-drag-handle" aria-label={t("Arrastar {panel}",{panel:t(PANEL_NAMES[panel.id])})} title={t("Arraste para mover")}
            onPointerDown={(event) => { if (event.button !== 0) return; event.preventDefault(); event.currentTarget.focus(); dragging.current = { id: panel.id, x: event.clientX, y: event.clientY, moved: false, target: panel.id, pointerId: event.pointerId, handle: event.currentTarget }; event.currentTarget.setPointerCapture(event.pointerId); }}
            onPointerMove={(event) => {
              const pending = dragging.current;
              if (!pending) return;
              if (Math.hypot(event.clientX - pending.x, event.clientY - pending.y) < 6) return;
              pending.moved = true;
              const target = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>("[data-dashboard-panel]")?.dataset.dashboardPanel;
              if (target) { pending.target = target; setDropTarget(target); }
            }}
            onPointerUp={(event) => { const pending = dragging.current; if (!pending) return; if (pending.moved) { const target = panels.findIndex((item) => item.id === pending.target); if (target >= 0) move(pending.id, target); } dragging.current = null; setDropTarget(null); event.currentTarget.releasePointerCapture(event.pointerId); }}
            onPointerCancel={cancelDrag} onLostPointerCapture={cancelDrag}>⠿</button>
          <button aria-label={t("Mover {panel} antes",{panel:t(PANEL_NAMES[panel.id])})} disabled={index === 0} onClick={() => move(panel.id, index - 1)}>←</button>
          <button aria-label={t("Mover {panel} depois",{panel:t(PANEL_NAMES[panel.id])})} disabled={index === panels.length - 1} onClick={() => move(panel.id, index + 1)}>→</button>
          <button aria-label={t(panel.wide ? "Compactar {panel}" : "Ampliar {panel}",{panel:t(PANEL_NAMES[panel.id])})} onClick={() => { setPanels((current) => current.map((item) => item.id === panel.id ? { ...item, wide: !item.wide } : item)); setNotice(`${PANEL_NAMES[panel.id]} redimensionado.`); }}>{panel.wide ? t("Compactar") : t("Ampliar")}</button>
          <button aria-label={t("Diminuir altura de {panel}",{panel:t(PANEL_NAMES[panel.id])})} disabled={panel.height <= 240} onClick={() => setHeight(panel.id, panel.height - 40)}>−</button>
          <button aria-label={t("Aumentar altura de {panel}",{panel:t(PANEL_NAMES[panel.id])})} disabled={panel.height >= 800} onClick={() => setHeight(panel.id, panel.height + 40)}>+</button>
        </div>}
        {children[panel.id]}
        {customizing && <button className="panel-resize-handle" aria-label={t("Redimensionar altura de {panel}",{panel:t(PANEL_NAMES[panel.id])})} title={t("Arraste ou use as setas para ajustar a altura")}
          onKeyDown={(event) => { if (["ArrowUp", "ArrowDown"].includes(event.key)) { event.preventDefault(); event.stopPropagation(); setHeight(panel.id, panel.height + (event.key === "ArrowUp" ? -20 : 20)); } }}
          onPointerDown={(event) => { if (event.button !== 0) return; event.preventDefault(); event.currentTarget.focus(); resize.current = { id: panel.id, y: event.clientY, original: panels, pointerId: event.pointerId, handle: event.currentTarget }; event.currentTarget.setPointerCapture(event.pointerId); }}
          onPointerMove={(event) => { const pending = resize.current; if (pending?.id !== panel.id) return; const initial = pending.original.find((item) => item.id === panel.id); if (initial) setHeight(panel.id, initial.height + event.clientY - pending.y); }}
          onPointerUp={(event) => { if (resize.current?.id !== panel.id) return; resize.current = null; event.currentTarget.releasePointerCapture(event.pointerId); setNotice(`${PANEL_NAMES[panel.id]} redimensionado.`); }}
          onPointerCancel={cancelResize} onLostPointerCapture={cancelResize}>↘</button>}
      </div>)}
    </div>
    <p className="muted" role="status">{storageError ? t("Layout disponível nesta sessão; não foi possível salvá-lo neste dispositivo.") : notice ? `${t(notice)} ${t("Alterações salvas neste dispositivo.")}` : customizing ? t("O layout é salvo automaticamente neste dispositivo.") : t("")}</p>
  </>;
}
