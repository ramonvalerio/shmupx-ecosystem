import { useCallback, useState } from "react";

/** Controles do painel do gabinete que o jogador mapeia (Configurações › Controle). Mesmos ids do Rust (`steam/controles.rs`). */
export type ControlId = "up" | "down" | "left" | "right" | "button1" | "button2" | "button3" | "button4" | "start" | "coin";

export const CONTROLS: { id: ControlId; label: string; short: string; fallback: string }[] = [
  { id: "up", label: "Cima", short: "↑", fallback: "↑" },
  { id: "down", label: "Baixo", short: "↓", fallback: "↓" },
  { id: "left", label: "Esquerda", short: "←", fallback: "←" },
  { id: "right", label: "Direita", short: "→", fallback: "→" },
  { id: "button1", label: "Botão 1", short: "1", fallback: "Ctrl" },
  { id: "button2", label: "Botão 2", short: "2", fallback: "Alt" },
  { id: "button3", label: "Botão 3", short: "3", fallback: "Espaço" },
  { id: "button4", label: "Botão 4", short: "4", fallback: "Shift" },
  { id: "start", label: "Start", short: "START", fallback: "1" },
  { id: "coin", label: "Ficha", short: "FICHA", fallback: "5" },
];

/** Botão, tecla ou direção que o motor leu para o controle. `code` é o código do MAME. */
export interface MappedControl {
  code: string;
  label: string;
  device?: string;
  deviceName?: string;
}

export type ControlMapping = Partial<Record<ControlId, MappedControl>>;

const KEY = "shmupx.emulator.controls";

export function readControlMapping(): ControlMapping {
  try {
    const saved = JSON.parse(window.localStorage.getItem(KEY) ?? "null") as ControlMapping | null;
    if (!saved || typeof saved !== "object") return {};
    const out: ControlMapping = {};
    for (const control of CONTROLS) {
      const item = saved[control.id];
      if (item && typeof item.code === "string" && typeof item.label === "string") out[control.id] = item;
    }
    return out;
  } catch {
    return {};
  }
}

/** Mapeamento no formato do comando `inicializar_jogo_emulador` (`motorControles`). */
export function readEmulatorControls(): { control: ControlId; code: string; device?: string }[] {
  const mapping = readControlMapping();
  return CONTROLS.flatMap((c) => {
    const item = mapping[c.id];
    return item ? [{ control: c.id, code: item.code, device: item.device }] : [];
  });
}

/** Nome do que foi apertado para mostrar ao jogador: "REAL ARCADE Pro.V3 · Botão 3", "Tecla Z". */
export function describeInput(item: MappedControl, t: (text: string, values?: Record<string, string | number>) => string): string {
  const joy = item.code.match(/^JOYCODE_\d+_(.+)$/);
  if (!joy) return t("Tecla {key}", { key: item.label });
  const part = joy[1];
  const button = part.match(/^BUTTON(\d+)$/);
  const what = button ? t("Botão {n}", { n: button[1] })
    : part.startsWith("HAT1") ? t("Direcional {dir}", { dir: { UP: "↑", DOWN: "↓", LEFT: "←", RIGHT: "→" }[part.slice(4)] ?? part })
    : part === "XAXIS_LEFT_SWITCH" ? t("Alavanca {dir}", { dir: "←" })
    : part === "XAXIS_RIGHT_SWITCH" ? t("Alavanca {dir}", { dir: "→" })
    : part === "YAXIS_UP_SWITCH" ? t("Alavanca {dir}", { dir: "↑" })
    : part === "YAXIS_DOWN_SWITCH" ? t("Alavanca {dir}", { dir: "↓" })
    : part === "START" ? "Start" : part === "SELECT" ? "Select" : item.label;
  return item.deviceName ? `${item.deviceName} · ${what}` : what;
}

/** Mapeamento com gravação automática neste computador. `saved` = false se o armazenamento falhou. */
export function useControlMapping() {
  const [mapping, setMapping] = useState<ControlMapping>(readControlMapping);
  const [saved, setSaved] = useState(true);
  const persist = useCallback((next: ControlMapping) => {
    setMapping(next);
    try {
      if (Object.keys(next).length === 0) window.localStorage.removeItem(KEY);
      else window.localStorage.setItem(KEY, JSON.stringify(next));
      setSaved(true);
    } catch {
      setSaved(false);
    }
  }, []);
  /** Dá o aperto ao controle; se outro controle usava o mesmo código, ele volta ao padrão. Devolve o controle que perdeu o código. */
  const assign = useCallback((control: ControlId, item: MappedControl): ControlId | null => {
    let moved: ControlId | null = null;
    const next: ControlMapping = {};
    for (const c of CONTROLS) {
      const current = mapping[c.id];
      if (c.id === control) next[c.id] = item;
      else if (current && current.code === item.code) moved = c.id;
      else if (current) next[c.id] = current;
    }
    persist(next);
    return moved;
  }, [mapping, persist]);
  const reset = useCallback(() => persist({}), [persist]);
  return { mapping, assign, reset, saved };
}
