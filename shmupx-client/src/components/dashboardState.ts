export const PANEL_IDS = ["performance", "games", "community", "ranking", "events"] as const;
export type PanelId = typeof PANEL_IDS[number];
export interface DashboardPanel { id: PanelId; wide: boolean; height: number; }
export const DASHBOARD_KEY = "shmupx.client.dashboard.v1";
export function defaultDashboard(): DashboardPanel[] {
  return PANEL_IDS.map((id) => ({ id, wide: id === "performance", height: id === "performance" || id === "games" ? 390 : 310 }));
}
export function validateDashboard(value: unknown): DashboardPanel[] | null {
  if (!Array.isArray(value) || value.length !== PANEL_IDS.length) return null;
  const result: DashboardPanel[] = [];
  for (const panel of value) {
    if (!panel || typeof panel !== "object" || !PANEL_IDS.includes(panel.id) || typeof panel.wide !== "boolean" || !Number.isFinite(panel.height) || panel.height < 240 || panel.height > 800 || result.some((item) => item.id === panel.id)) return null;
    result.push({ id: panel.id, wide: panel.wide, height: panel.height });
  }
  return result;
}
export function reorderPanel(panels: DashboardPanel[], id: PanelId, target: number): DashboardPanel[] {
  const index = panels.findIndex((panel) => panel.id === id);
  if (index < 0 || target < 0 || target >= panels.length || target === index) return panels;
  const next = [...panels];
  const [panel] = next.splice(index, 1);
  next.splice(target, 0, panel);
  return next;
}
