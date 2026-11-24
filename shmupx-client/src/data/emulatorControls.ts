import { useCallback, useState } from "react";

/** Controles do painel do gabinete que o jogador mapeia (Configurações › Controle). Mesmos ids do Rust (`steam/controles.rs`). */
export type ControlId = "up" | "down" | "left" | "right" | "button1" | "button2" | "button3" | "button4" | "start" | "coin";

/**
 * `defaults`: a sequência padrão do MAME para o jogador 1, a mesma de `CONTROLES` no Rust
 * (`steam/controles.rs`): vale enquanto o controle não foi mapeado.
 */
export const CONTROLS: { id: ControlId; label: string; short: string; fallback: string; defaults: string[] }[] = [
  { id: "up", label: "Cima", short: "↑", fallback: "↑", defaults: ["KEYCODE_UP", "JOYCODE_1_YAXIS_UP_SWITCH", "JOYCODE_1_HAT1UP"] },
  { id: "down", label: "Baixo", short: "↓", fallback: "↓", defaults: ["KEYCODE_DOWN", "JOYCODE_1_YAXIS_DOWN_SWITCH", "JOYCODE_1_HAT1DOWN"] },
  { id: "left", label: "Esquerda", short: "←", fallback: "←", defaults: ["KEYCODE_LEFT", "JOYCODE_1_XAXIS_LEFT_SWITCH", "JOYCODE_1_HAT1LEFT"] },
  { id: "right", label: "Direita", short: "→", fallback: "→", defaults: ["KEYCODE_RIGHT", "JOYCODE_1_XAXIS_RIGHT_SWITCH", "JOYCODE_1_HAT1RIGHT"] },
  { id: "button1", label: "Botão 1", short: "1", fallback: "Ctrl", defaults: ["KEYCODE_LCONTROL", "JOYCODE_1_BUTTON1"] },
  { id: "button2", label: "Botão 2", short: "2", fallback: "Alt", defaults: ["KEYCODE_LALT", "JOYCODE_1_BUTTON2"] },
  { id: "button3", label: "Botão 3", short: "3", fallback: "Espaço", defaults: ["KEYCODE_SPACE", "JOYCODE_1_BUTTON3"] },
  { id: "button4", label: "Botão 4", short: "4", fallback: "Shift", defaults: ["KEYCODE_LSHIFT", "JOYCODE_1_BUTTON4"] },
  { id: "start", label: "Start", short: "START", fallback: "1", defaults: ["KEYCODE_1", "JOYCODE_1_START"] },
  { id: "coin", label: "Ficha", short: "FICHA", fallback: "5", defaults: ["KEYCODE_5", "JOYCODE_1_SELECT"] },
];

/**
 * Controles do gabinete acionados pelos códigos apertados agora, como no jogo: o controle mapeado
 * responde só ao código escolhido; o não mapeado, ao padrão do MAME sem os códigos que foram dados
 * a outro controle.
 */
export function pressedControls(mapping: ControlMapping, pressed: ReadonlySet<string>): Set<ControlId> {
  const out = new Set<ControlId>();
  if (pressed.size === 0) return out;
  const used = new Set(CONTROLS.flatMap((c) => (mapping[c.id] ? [mapping[c.id]!.code] : [])));
  for (const control of CONTROLS) {
    const item = mapping[control.id];
    const codes = item ? [item.code] : control.defaults.filter((code) => !used.has(code));
    if (codes.some((code) => pressed.has(code))) out.add(control.id);
  }
  return out;
}

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
    : /^(\w+AXIS|SLIDER\d)_(POS|NEG)_SWITCH$/.test(part)
      ? t("Eixo {axis} {dir}", { axis: part.replace(/AXIS_.*|_(POS|NEG)_SWITCH$/g, ""), dir: part.includes("_POS_") ? "+" : "−" })
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
