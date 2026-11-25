import { useCallback, useEffect, useState } from "react";

/**
 * Controles que o jogador já usou neste PC e se estão plugados agora (Configurações › Controle).
 * "Plugado" vem da Gamepad API do WebView (avisa ao plugar/desplugar) e do próprio motor, que manda
 * o nome do aparelho a cada aperto. Os já vistos ficam salvos para aparecerem como desconectados.
 */
export interface KnownController {
  name: string;
  connected: boolean;
}

/** Escolha da tela: todos (controle e teclado), só o teclado ou um controle pelo nome. */
export type ControllerChoice = { kind: "all" } | { kind: "keyboard" } | { kind: "pad"; name: string };

const KNOWN_KEY = "shmupx.emulator.knownControllers";
const CHOICE_KEY = "shmupx.emulator.controllerChoice";

/** Nome do controle sem o trecho técnico do id da Gamepad API ("… (Vendor: 0f0d Product: 0022)"). */
export function padName(id: string): string {
  return id.replace(/\s*\(.*$/, "").trim() || id.trim();
}

/** Mesmo aparelho: nomes iguais, ou um contido no outro (o motor e o WebView escrevem diferente). */
export function sameController(a: string, b: string): boolean {
  const x = a.trim().toLowerCase();
  const y = b.trim().toLowerCase();
  if (!x || !y) return false;
  return x === y || x.includes(y) || y.includes(x);
}

function readKnown(): string[] {
  try {
    const saved = JSON.parse(window.localStorage.getItem(KNOWN_KEY) ?? "[]");
    return Array.isArray(saved) ? saved.filter((name): name is string => typeof name === "string" && name.trim() !== "") : [];
  } catch {
    return [];
  }
}

function writeKnown(names: string[]) {
  try { window.localStorage.setItem(KNOWN_KEY, JSON.stringify(names)); } catch { /* sem armazenamento: vale só nesta sessão */ }
}

export function readControllerChoice(): ControllerChoice {
  try {
    const saved = JSON.parse(window.localStorage.getItem(CHOICE_KEY) ?? "null") as ControllerChoice | null;
    if (saved?.kind === "keyboard") return saved;
    if (saved?.kind === "pad" && typeof saved.name === "string" && saved.name) return saved;
  } catch { /* padrão abaixo */ }
  return { kind: "all" };
}

function browserPads(): string[] {
  try {
    return Array.from(navigator.getGamepads?.() ?? [])
      .filter((pad): pad is Gamepad => !!pad && pad.connected)
      .map((pad) => padName(pad.id));
  } catch {
    return [];
  }
}

function merge(list: string[], name: string): string[] {
  return list.some((known) => sameController(known, name)) ? list : [...list, name];
}

/**
 * Controles conhecidos com a situação de agora. `seen(name)`: o motor leu um aperto desse aparelho
 * (conta como plugado até o WebView avisar que saiu).
 */
export function useConnectedControllers(extra: (string | undefined)[] = []) {
  const [known, setKnown] = useState<string[]>(readKnown);
  const [pads, setPads] = useState<string[]>(browserPads);
  const [seenNow, setSeenNow] = useState<string[]>([]);

  useEffect(() => {
    const refresh = () => setPads((current) => {
      const next = browserPads();
      return next.length === current.length && next.every((name, i) => name === current[i]) ? current : next;
    });
    const onDisconnect = (event: GamepadEvent) => {
      const name = padName(event.gamepad.id);
      setSeenNow((current) => current.filter((seen) => !sameController(seen, name)));
      refresh();
    };
    window.addEventListener("gamepadconnected", refresh);
    window.addEventListener("gamepaddisconnected", onDisconnect);
    // Alguns WebViews só avisam a conexão depois do primeiro aperto; a consulta periódica cobre o resto.
    const timer = window.setInterval(refresh, 1000);
    return () => {
      window.removeEventListener("gamepadconnected", refresh);
      window.removeEventListener("gamepaddisconnected", onDisconnect);
      window.clearInterval(timer);
    };
  }, []);

  // Todo controle que aparecer (WebView, motor ou mapeamento salvo) entra na lista e fica lembrado.
  const extraKey = extra.filter(Boolean).join("\u0000");
  useEffect(() => {
    setKnown((current) => {
      let next = current;
      for (const name of [...pads, ...seenNow, ...extraKey.split("\u0000")]) if (name) next = merge(next, name);
      if (next !== current) writeKnown(next);
      return next;
    });
  }, [pads, seenNow, extraKey]);

  const seen = useCallback((name: string | undefined) => {
    if (name) setSeenNow((current) => merge(current, name));
  }, []);

  const controllers: KnownController[] = known.map((name) => ({
    name,
    connected: [...pads, ...seenNow].some((live) => sameController(live, name)),
  }));
  return { controllers, seen };
}

/** Escolha do controle gravada neste computador. */
export function useControllerChoice() {
  const [choice, setChoice] = useState<ControllerChoice>(readControllerChoice);
  const choose = useCallback((next: ControllerChoice) => {
    setChoice(next);
    try { window.localStorage.setItem(CHOICE_KEY, JSON.stringify(next)); } catch { /* vale só nesta sessão */ }
  }, []);
  return { choice, choose };
}

/** O aperto vem do controle escolhido? Esc (cancelar) passa sempre. */
export function acceptsInput(choice: ControllerChoice, code: string, deviceName: string | undefined): boolean {
  if (choice.kind === "all" || code === "KEYCODE_ESC") return true;
  const fromPad = code.startsWith("JOYCODE_");
  if (choice.kind === "keyboard") return !fromPad;
  return fromPad && (!deviceName || sameController(choice.name, deviceName));
}
