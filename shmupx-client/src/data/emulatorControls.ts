import { useCallback, useMemo, useState, useSyncExternalStore } from "react";

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
 * responde aos códigos escolhidos (um por aparelho); o não mapeado, ao padrão do MAME sem os códigos
 * que foram dados a outro controle.
 */
export function pressedControls(mapping: MergedMapping, pressed: ReadonlySet<string>): Set<ControlId> {
  const out = new Set<ControlId>();
  if (pressed.size === 0) return out;
  const used = new Set(CONTROLS.flatMap((c) => (mapping[c.id] ?? []).map((item) => item.code)));
  for (const control of CONTROLS) {
    const items = mapping[control.id] ?? [];
    const codes = items.length ? items.map((item) => item.code) : control.defaults.filter((code) => !used.has(code));
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

/** Mapeamento de um aparelho (o teclado ou um controle): um código por controle do gabinete. */
export type ControlMapping = Partial<Record<ControlId, MappedControl>>;
/** Todos os aparelhos juntos: os códigos de cada controle do gabinete. */
export type MergedMapping = Partial<Record<ControlId, MappedControl[]>>;
/** Mapeamentos por aparelho: "keyboard" ou "pad:<nome do controle>". */
export type ControlProfiles = Record<string, ControlMapping>;

const KEY = "shmupx.emulator.controls";
const PROFILES_KEY = "shmupx.emulator.controls.v2";

/** Aparelho do aperto: o teclado ou o controle (pelo nome). */
export function profileOf(item: Pick<MappedControl, "code" | "device" | "deviceName">): string {
  return item.code.startsWith("JOYCODE_") ? `pad:${item.deviceName || item.device || "?"}` : "keyboard";
}

function validMapping(saved: unknown): ControlMapping {
  const out: ControlMapping = {};
  if (!saved || typeof saved !== "object") return out;
  for (const control of CONTROLS) {
    const item = (saved as Record<string, MappedControl | undefined>)[control.id];
    if (item && typeof item.code === "string" && typeof item.label === "string") out[control.id] = item;
  }
  return out;
}

/** Mapeamentos gravados. O formato antigo (um só para todos) vira um por aparelho. */
export function readControlProfiles(): ControlProfiles {
  try {
    const saved = JSON.parse(window.localStorage.getItem(PROFILES_KEY) ?? "null") as Record<string, unknown> | null;
    if (saved && typeof saved === "object") {
      const out: ControlProfiles = {};
      for (const [key, mapping] of Object.entries(saved)) {
        const valid = validMapping(mapping);
        if (Object.keys(valid).length) out[key] = valid;
      }
      return out;
    }
    const old = validMapping(JSON.parse(window.localStorage.getItem(KEY) ?? "null"));
    const out: ControlProfiles = {};
    for (const control of CONTROLS) {
      const item = old[control.id];
      if (item) (out[profileOf(item)] ??= {})[control.id] = item;
    }
    return out;
  } catch {
    return {};
  }
}

/** Todos os aparelhos juntos (o gabinete responde a qualquer um deles). */
export function mergeProfiles(profiles: ControlProfiles): MergedMapping {
  const out: MergedMapping = {};
  for (const mapping of Object.values(profiles)) {
    for (const control of CONTROLS) {
      const item = mapping[control.id];
      if (item) (out[control.id] ??= []).push(item);
    }
  }
  return out;
}

/** Mapeamento no formato do comando `inicializar_jogo_emulador` (`motorControles`): todos os aparelhos. */
export function readEmulatorControls(): { control: ControlId; code: string; device?: string }[] {
  const merged = mergeProfiles(readControlProfiles());
  return CONTROLS.flatMap((c) => (merged[c.id] ?? []).map((item) => ({ control: c.id, code: item.code, device: item.device })));
}

/** Nome do que foi apertado para mostrar ao jogador: "Botão 3", "Tecla Z" (o controle aparece na escolha acima da lista). */
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
  return what;
}

/** Mapeamentos atuais, um só estado no Client: o gabinete da Coleção vê na hora o que a tela de controles grava. */
let currentProfiles: ControlProfiles | null = null;
const mappingListeners = new Set<() => void>();
const profilesSnapshot = () => (currentProfiles ??= readControlProfiles());
const subscribeMapping = (listener: () => void) => {
  mappingListeners.add(listener);
  return () => { mappingListeners.delete(listener); };
};

/**
 * Mapeamentos por aparelho, com gravação automática neste computador: mapear um controle não mexe
 * no do teclado nem no de outro controle. `merged`: todos juntos. `saved` = false se o armazenamento falhou.
 */
export function useControlMapping() {
  const profiles = useSyncExternalStore(subscribeMapping, profilesSnapshot);
  const merged = useMemo(() => mergeProfiles(profiles), [profiles]);
  const [saved, setSaved] = useState(true);
  const persist = useCallback((next: ControlProfiles) => {
    currentProfiles = next;
    for (const listener of mappingListeners) listener();
    try {
      if (Object.keys(next).length === 0) window.localStorage.removeItem(PROFILES_KEY);
      else window.localStorage.setItem(PROFILES_KEY, JSON.stringify(next));
      window.localStorage.removeItem(KEY);
      setSaved(true);
    } catch {
      setSaved(false);
    }
  }, []);
  /** Dá o aperto ao controle no mapeamento do aparelho dele; se outro controle desse aparelho usava o mesmo código, ele volta ao padrão. Devolve o controle que perdeu o código. */
  const assign = useCallback((control: ControlId, item: MappedControl): ControlId | null => {
    const key = profileOf(item);
    const mapping = profiles[key] ?? {};
    let moved: ControlId | null = null;
    const next: ControlMapping = {};
    for (const c of CONTROLS) {
      const current = mapping[c.id];
      if (c.id === control) next[c.id] = item;
      else if (current && current.code === item.code) moved = c.id;
      else if (current) next[c.id] = current;
    }
    persist({ ...profiles, [key]: next });
    return moved;
  }, [profiles, persist]);
  /** Volta ao padrão: os aparelhos indicados (chaves de `profiles`), ou todos. */
  const reset = useCallback((keys?: string[]) => {
    if (!keys) { persist({}); return; }
    const next = { ...profiles };
    for (const key of keys) delete next[key];
    persist(next);
  }, [profiles, persist]);
  return { profiles, merged, assign, reset, saved };
}
