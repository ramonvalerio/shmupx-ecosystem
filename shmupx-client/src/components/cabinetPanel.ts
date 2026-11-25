import { useEffect, useRef, useState } from "react";
import { pressedControls, type ControlId, type MergedMapping } from "../data/emulatorControls";

/**
 * Painel do gabinete ao vivo: a alavanca e os botões acompanham o controle ou o teclado do jogador.
 * Usado pelo gabinete grande de Configurações › Controle e pelo gabinete de Minhas coleções.
 */

type Point = { x: number; y: number };
type Stick = { x: number; y: number };
/** Controle cujo analógico a alavanca segue; `name` é o nome que o motor deu ao aparelho. */
type StickDevice = { name: string | null };

/** Onde a bola da alavanca fica em repouso (coordenadas do SVG) e quanto ela anda até o fim. */
export interface LeverGeometry {
  base: Point;
  ball: Point;
  travel: number;
}

/** Códigos do MAME das direções pela alavanca do controle (eixos X/Y como chave). */
export const AXIS_SWITCH = /^JOYCODE_\d+_[XY]AXIS_\w+_SWITCH$/;
const DIRECTIONS = ["up", "down", "left", "right"] as const;
/** Zona morta do analógico (tremida em repouso) e ponto em que a direção conta como apertada. */
const STICK_DEADZONE = 0.12;
const STICK_SWITCH = 0.5;

/**
 * De qual controle ler o analógico: o das direções mapeadas pela alavanca (eixos X/Y). Direção sem
 * mapeamento usa o padrão do MAME, que inclui a alavanca do controle; tudo no teclado ou no
 * direcional digital, nenhum (aí vale só o que o motor avisa).
 */
function stickDevice(mapping: MergedMapping): StickDevice | null {
  const axis = DIRECTIONS.flatMap((id) => mapping[id] ?? []).find((item) => AXIS_SWITCH.test(item.code));
  if (axis) return { name: axis.deviceName ?? null };
  return DIRECTIONS.some((id) => !mapping[id]?.length) ? { name: null } : null;
}

/**
 * Posição do analógico pela Gamepad API do WebView, em -1..1 dentro do círculo (a diagonal anda o
 * mesmo tanto que uma direção só). `null` sem controle à vista. Com vários controles, vale o de
 * mesmo nome do mapeado; entre eles (ou sem nome), o que está mais inclinado.
 */
function readStick(device: StickDevice): Stick | null {
  let pads: Gamepad[];
  try {
    pads = Array.from(navigator.getGamepads?.() ?? []).filter((pad): pad is Gamepad => !!pad && pad.connected && pad.axes.length >= 2);
  } catch {
    return null;
  }
  if (pads.length === 0) return null;
  const wanted = device.name?.trim().toLowerCase();
  const named = wanted
    ? pads.filter((pad) => {
        const id = pad.id.toLowerCase();
        const base = id.replace(/\s*\(.*$/, "").trim();
        return id.includes(wanted) || (base.length > 0 && wanted.includes(base));
      })
    : [];
  let best = { x: 0, y: 0, mag: 0 };
  for (const pad of named.length ? named : pads) {
    const x = Number.isFinite(pad.axes[0]) ? pad.axes[0] : 0;
    const y = Number.isFinite(pad.axes[1]) ? pad.axes[1] : 0;
    const mag = Math.hypot(x, y);
    if (mag > best.mag) best = { x, y, mag };
  }
  if (best.mag < STICK_DEADZONE) return { x: 0, y: 0 };
  const scaled = Math.min(1, (best.mag - STICK_DEADZONE) / (1 - STICK_DEADZONE));
  return { x: (best.x / best.mag) * scaled, y: (best.y / best.mag) * scaled };
}

function stickDirections(stick: Stick): string {
  return [
    stick.y < -STICK_SWITCH ? "up" : "",
    stick.y > STICK_SWITCH ? "down" : "",
    stick.x < -STICK_SWITCH ? "left" : "",
    stick.x > STICK_SWITCH ? "right" : "",
  ].filter(Boolean).join(",");
}

/**
 * Alavanca do painel. Com o analógico do controle à vista, a bola segue o eixo quadro a quadro, em
 * qualquer ângulo (girar o direcional faz a alavanca girar junto). Sem ele (teclado, direcional
 * digital), mola: a bola vai até a direção apertada (e diagonais) e volta ao centro ao soltar, com
 * uma leve sobra no fim. Anima direto no SVG (sem re-render a cada quadro); com "reduzir movimento"
 * do sistema, a mola só pula para a posição. `onStick` avisa quando o controle aparece/some e quando
 * as direções do analógico mudam.
 */
function useLever(target: Stick, device: StickDevice | null, geometry: LeverGeometry, onStick: (live: boolean, dirs: string) => void) {
  const ball = useRef<SVGGElement>(null);
  const shaft = useRef<SVGLineElement>(null);
  const state = useRef({ x: 0, y: 0, vx: 0, vy: 0, tx: 0, ty: 0, frame: 0, last: 0, live: false, dirs: "" });
  const deviceRef = useRef(device);
  deviceRef.current = device;
  const geometryRef = useRef(geometry);
  geometryRef.current = geometry;
  const onStickRef = useRef(onStick);
  onStickRef.current = onStick;
  const kick = useRef<() => void>(() => undefined);

  useEffect(() => {
    const st = state.current;
    const reduced = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    // Mola um pouco abaixo do amortecimento crítico: rápida, com uma sobra pequena.
    const STIFFNESS = 1100;
    const DAMPING = 2 * Math.sqrt(STIFFNESS) * 0.72;
    const draw = () => {
      const { ball: rest } = geometryRef.current;
      ball.current?.setAttribute("transform", `translate(${st.x.toFixed(2)} ${st.y.toFixed(2)})`);
      shaft.current?.setAttribute("x2", (rest.x + st.x).toFixed(2));
      shaft.current?.setAttribute("y2", (rest.y + st.y).toFixed(2));
    };
    const step = (now: number) => {
      const dt = Math.min(0.032, (now - st.last) / 1000);
      st.last = now;
      const travel = geometryRef.current.travel;
      const stick = deviceRef.current ? readStick(deviceRef.current) : null;
      const live = stick !== null;
      const dirs = stick ? stickDirections(stick) : "";
      if (live !== st.live || dirs !== st.dirs) {
        st.live = live;
        st.dirs = dirs;
        onStickRef.current(live, dirs);
      }
      if (stick && (stick.x || stick.y)) {
        // O analógico manda: a bola fica exatamente onde o eixo está.
        Object.assign(st, { x: stick.x * travel, y: stick.y * travel, vx: 0, vy: 0 });
      } else if (reduced) {
        Object.assign(st, { x: st.tx, y: st.ty, vx: 0, vy: 0 });
      } else {
        for (const [p, v, t] of [["x", "vx", "tx"], ["y", "vy", "ty"]] as const) {
          const accel = STIFFNESS * (st[t] - st[p]) - DAMPING * st[v];
          st[v] += accel * dt;
          st[p] += st[v] * dt;
        }
      }
      const settle = travel / 240;
      const resting = Math.abs(st.tx - st.x) < settle && Math.abs(st.ty - st.y) < settle && Math.abs(st.vx) < travel / 12 && Math.abs(st.vy) < travel / 12;
      if (resting && !(stick && (stick.x || stick.y))) Object.assign(st, { x: st.tx, y: st.ty, vx: 0, vy: 0 });
      draw();
      // Com controle à vista, lê o analógico todo quadro; sem ele, para quando a mola assenta.
      st.frame = live || !resting ? requestAnimationFrame(step) : 0;
    };
    kick.current = () => {
      if (st.frame) return;
      st.last = performance.now();
      st.frame = requestAnimationFrame(step);
    };
    // O WebView só mostra o controle depois do primeiro aperto nele: aí o evento acorda a leitura.
    const onConnect = () => kick.current();
    window.addEventListener("gamepadconnected", onConnect);
    window.addEventListener("gamepaddisconnected", onConnect);
    kick.current();
    return () => {
      window.removeEventListener("gamepadconnected", onConnect);
      window.removeEventListener("gamepaddisconnected", onConnect);
      cancelAnimationFrame(st.frame);
      st.frame = 0;
    };
  }, []);

  useEffect(() => {
    state.current.tx = target.x;
    state.current.ty = target.y;
    kick.current();
  }, [target.x, target.y]);

  // Mapeamento mudou (outro controle ou alavanca no teclado): volta a ler.
  useEffect(() => kick.current(), [device?.name, device === null]);

  return { ball, shaft };
}

/**
 * Controles acionados agora (botões afundam, setas acendem) e a alavanca animada. `held`: os códigos
 * do MAME apertados (motor de captura). Com `enabled` falso, o painel fica parado em repouso.
 */
export function useCabinetPanel(mapping: MergedMapping, held: ReadonlySet<string>, geometry: LeverGeometry, enabled = true) {
  // Com o analógico lido pelo WebView, ele manda na alavanca e nas setas: os apertos de eixo que o
  // motor avisa ficam de fora (chegam um pouco depois e fariam a bola pular para o fim).
  const [stick, setStick] = useState<{ live: boolean; dirs: string }>({ live: false, dirs: "" });
  const device = enabled ? stickDevice(mapping) : null;
  const stickLive = stick.live && device !== null;
  const codes = enabled ? held : new Set<string>();
  const digital = pressedControls(mapping, stickLive ? new Set([...codes].filter((code) => !AXIS_SWITCH.test(code))) : codes);
  const pressed = new Set<ControlId>(digital);
  if (stickLive) for (const dir of stick.dirs.split(",")) if (dir) pressed.add(dir as ControlId);
  const dx = (digital.has("right") ? 1 : 0) - (digital.has("left") ? 1 : 0);
  const dy = (digital.has("down") ? 1 : 0) - (digital.has("up") ? 1 : 0);
  // Na diagonal, a bola anda o mesmo tanto que numa direção só (alavanca de 8 direções).
  const scale = dx && dy ? geometry.travel / Math.SQRT2 : geometry.travel;
  const lever = useLever({ x: dx * scale, y: dy * scale }, device, geometry, (live, dirs) => setStick({ live, dirs }));
  return { pressed, lever };
}
