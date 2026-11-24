import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "../i18n";
import { CONTROLS, describeInput, pressedControls, useControlMapping, type ControlId, type ControlMapping, type MappedControl } from "../data/emulatorControls";

/** Evento do motor em modo captura (Rust `steam/controles.rs`). */
interface CaptureEvent {
  kind: "ready" | "input" | "release" | "ended";
  code?: string;
  label?: string;
  device?: string;
  deviceName?: string;
}

type EngineState = "starting" | "ready" | "unavailable";

/**
 * Posição de cada controle no painel (coordenadas do SVG). O gabinete segue o desenho do
 * `ArcadeCabinet` (o mesmo da Coleção, em dobro), com o painel um pouco mais fundo para os
 * botões ficarem fáceis de clicar.
 */
const SPOTS: Record<ControlId, { x: number; y: number; r: number }> = {
  up: { x: 186, y: 452, r: 14 },
  down: { x: 186, y: 520, r: 14 },
  left: { x: 150, y: 486, r: 14 },
  right: { x: 222, y: 486, r: 14 },
  start: { x: 288, y: 454, r: 9 },
  button1: { x: 330, y: 502, r: 18 },
  button2: { x: 384, y: 488, r: 18 },
  button3: { x: 438, y: 482, r: 18 },
  button4: { x: 492, y: 482, r: 18 },
  coin: { x: 462, y: 590, r: 0 },
};
const ARROW: Partial<Record<ControlId, number>> = { up: 0, right: 90, down: 180, left: 270 };

/** Centro da base da alavanca e da bola em repouso (a bola fica um pouco acima, pela perspectiva). */
const LEVER_BASE = { x: 186, y: 486 };
const LEVER_BALL = { x: 185, y: 482 };
/** Quanto a bola anda ao inclinar a alavanca até o fim. */
const LEVER_TRAVEL = 12;

type Stick = { x: number; y: number };
/** Controle cujo analógico a alavanca segue; `name` é o nome que o motor deu ao aparelho. */
type StickDevice = { name: string | null };
/** Códigos do MAME das direções pela alavanca do controle (eixos X/Y como chave). */
const AXIS_SWITCH = /^JOYCODE_\d+_[XY]AXIS_\w+_SWITCH$/;
const DIRECTIONS = ["up", "down", "left", "right"] as const;
/** Zona morta do analógico (tremida em repouso) e ponto em que a direção conta como apertada. */
const STICK_DEADZONE = 0.12;
const STICK_SWITCH = 0.5;

/**
 * De qual controle ler o analógico: o das direções mapeadas pela alavanca (eixos X/Y). Direção sem
 * mapeamento usa o padrão do MAME, que inclui a alavanca do controle; tudo no teclado ou no
 * direcional digital, nenhum (aí vale só o que o motor avisa).
 */
function stickDevice(mapping: ControlMapping): StickDevice | null {
  const axis = DIRECTIONS.map((id) => mapping[id]).find((item) => item && AXIS_SWITCH.test(item.code));
  if (axis) return { name: axis.deviceName ?? null };
  return DIRECTIONS.some((id) => !mapping[id]) ? { name: null } : null;
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
function useLever(target: Stick, device: StickDevice | null, onStick: (live: boolean, dirs: string) => void) {
  const ball = useRef<SVGGElement>(null);
  const shaft = useRef<SVGLineElement>(null);
  const state = useRef({ x: 0, y: 0, vx: 0, vy: 0, tx: 0, ty: 0, frame: 0, last: 0, live: false, dirs: "" });
  const deviceRef = useRef(device);
  deviceRef.current = device;
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
      ball.current?.setAttribute("transform", `translate(${st.x.toFixed(2)} ${st.y.toFixed(2)})`);
      shaft.current?.setAttribute("x2", (LEVER_BALL.x + st.x).toFixed(2));
      shaft.current?.setAttribute("y2", (LEVER_BALL.y + st.y).toFixed(2));
    };
    const step = (now: number) => {
      const dt = Math.min(0.032, (now - st.last) / 1000);
      st.last = now;
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
        Object.assign(st, { x: stick.x * LEVER_TRAVEL, y: stick.y * LEVER_TRAVEL, vx: 0, vy: 0 });
      } else if (reduced) {
        Object.assign(st, { x: st.tx, y: st.ty, vx: 0, vy: 0 });
      } else {
        for (const [p, v, t] of [["x", "vx", "tx"], ["y", "vy", "ty"]] as const) {
          const accel = STIFFNESS * (st[t] - st[p]) - DAMPING * st[v];
          st[v] += accel * dt;
          st[p] += st[v] * dt;
        }
      }
      const resting = Math.abs(st.tx - st.x) < 0.05 && Math.abs(st.ty - st.y) < 0.05 && Math.abs(st.vx) < 1 && Math.abs(st.vy) < 1;
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
 * Gabinete arcade grande para mapear os controles do ShmupX Emulator: clique num botão do painel
 * (ou na lista) e aperte o botão, tecla ou direção que quer usar. Grava na hora, sem "Salvar".
 * A leitura é do próprio motor, escondido, para o código ser o que o jogo usa.
 */
export default function CabinetControlMapper() {
  const { t } = useI18n();
  const { mapping, assign, reset, saved } = useControlMapping();
  const [engine, setEngine] = useState<EngineState>("starting");
  const [waiting, setWaiting] = useState<ControlId | null>(null);
  const [flash, setFlash] = useState<{ control: ControlId; text: string } | null>(null);
  const [hover, setHover] = useState<ControlId | null>(null);
  /** Códigos apertados agora no controle ou teclado (o motor avisa aperto e soltura). */
  const [held, setHeld] = useState<ReadonlySet<string>>(() => new Set());
  const waitingRef = useRef<ControlId | null>(null);
  waitingRef.current = waiting;
  const assignRef = useRef(assign);
  assignRef.current = assign;
  const tRef = useRef(t);
  tRef.current = t;

  // Motor em modo captura enquanto a tela está aberta; se ele fechar (Esc no motor), abre de novo.
  useEffect(() => {
    if (!isTauri()) { setEngine("unavailable"); return; }
    let alive = true;
    let restarts = 0;
    let stop: (() => void) | undefined;
    const start = () => invoke("motor_captura_iniciar").catch(() => { if (alive) setEngine("unavailable"); });
    void listen<CaptureEvent>("engine://capture", (event) => {
      if (!alive) return;
      const data = event.payload;
      if (data.kind === "ready") { setEngine("ready"); restarts = 0; return; }
      if (data.kind === "ended") {
        setHeld(new Set());
        if (restarts++ < 3) { setEngine("starting"); void start(); } else setEngine("unavailable");
        return;
      }
      if (data.kind === "release") {
        if (data.code) setHeld((current) => { const next = new Set(current); next.delete(data.code!); return next; });
        return;
      }
      if (data.code) setHeld((current) => new Set(current).add(data.code!));
      const control = waitingRef.current;
      if (!control || !data.code) return;
      if (data.code === "KEYCODE_ESC") { setWaiting(null); return; }
      const item: MappedControl = { code: data.code, label: data.label ?? data.code, device: data.device, deviceName: data.deviceName };
      const moved = assignRef.current(control, item);
      setWaiting(null);
      const name = CONTROLS.find((c) => c.id === control)!.label;
      const text = moved
        ? tRef.current("{control}: {input}. Saiu de {other}, que voltou ao padrão.", { control: tRef.current(name), input: describeInput(item, tRef.current), other: tRef.current(CONTROLS.find((c) => c.id === moved)!.label) })
        : tRef.current("{control}: {input}", { control: tRef.current(name), input: describeInput(item, tRef.current) });
      setFlash({ control, text });
    }).then((unlisten) => { if (alive) stop = unlisten; else unlisten(); });
    void start();
    return () => {
      alive = false;
      stop?.();
      void invoke("motor_captura_parar").catch(() => undefined);
    };
  }, []);

  // Esperando o aperto: as teclas não mexem na tela (Espaço e Enter não clicam de novo); Esc cancela.
  useEffect(() => {
    if (!waiting) return;
    const onKey = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.key === "Escape") setWaiting(null);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [waiting]);

  useEffect(() => {
    if (!flash) return;
    const timer = window.setTimeout(() => setFlash(null), 4000);
    return () => window.clearTimeout(timer);
  }, [flash]);

  const pick = (control: ControlId) => {
    if (engine !== "ready") return;
    setFlash(null);
    setWaiting((current) => (current === control ? null : control));
  };
  const labelOf = (id: ControlId) => t(CONTROLS.find((c) => c.id === id)!.label);
  const bindingOf = (id: ControlId) => {
    const item = mapping[id];
    return item ? describeInput(item, t) : t("Padrão ({key})", { key: t(CONTROLS.find((c) => c.id === id)!.fallback) });
  };

  const screenLines: string[] =
    engine === "unavailable" ? [t("SHMUPX EMULATOR"), t("NÃO ENCONTRADO")]
    : engine === "starting" ? [t("PREPARANDO"), t("OS CONTROLES…")]
    : waiting ? [t("APERTE O BOTÃO PARA"), labelOf(waiting).toUpperCase()]
    : flash ? [t("GRAVADO!"), labelOf(flash.control).toUpperCase()]
    : [t("CLIQUE NUM BOTÃO"), t("DO PAINEL")];

  // Com o analógico lido pelo WebView, ele manda na alavanca e nas setas: os apertos de eixo que o
  // motor avisa ficam de fora (chegam um pouco depois e fariam a bola pular para o fim).
  const [stick, setStick] = useState<{ live: boolean; dirs: string }>({ live: false, dirs: "" });
  const device = stickDevice(mapping);
  const stickLive = stick.live && device !== null;
  const digital = pressedControls(mapping, stickLive ? new Set([...held].filter((code) => !AXIS_SWITCH.test(code))) : held);
  const pressed = new Set(digital);
  if (stickLive) for (const dir of stick.dirs.split(",")) if (dir) pressed.add(dir as ControlId);
  const lever = useLever((() => {
    const dx = (digital.has("right") ? 1 : 0) - (digital.has("left") ? 1 : 0);
    const dy = (digital.has("down") ? 1 : 0) - (digital.has("up") ? 1 : 0);
    // Na diagonal, a bola anda o mesmo tanto que numa direção só (alavanca de 8 direções).
    const scale = dx && dy ? LEVER_TRAVEL / Math.SQRT2 : LEVER_TRAVEL;
    return { x: dx * scale, y: dy * scale };
  })(), device, (live, dirs) => setStick({ live, dirs }));

  const spotClass = (id: ControlId) => `mapper-spot ${pressed.has(id) ? "is-pressed" : ""} ${waiting === id ? "is-waiting" : ""} ${flash?.control === id ? "is-saved" : ""} ${hover === id ? "is-hover" : ""} ${mapping[id] ? "is-mapped" : ""}`;
  const spotProps = (id: ControlId) => ({
    className: spotClass(id),
    role: "button",
    tabIndex: -1,
    "aria-label": `${labelOf(id)}: ${bindingOf(id)}`,
    onClick: () => pick(id),
    onMouseEnter: () => setHover(id),
    onMouseLeave: () => setHover(null),
  });

  return (
    <div className={`cabinet-mapper ${engine !== "ready" ? "is-busy" : ""}`}>
      <div className="cabinet-mapper-artbox">
      <svg className="cabinet-mapper-art" viewBox="44 0 552 654" preserveAspectRatio="xMidYMid meet" role="img" aria-label={t("Gabinete do ShmupX")}>
        <defs>
          <linearGradient id="mapper-body" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stopColor="#b9c0c6" /><stop offset=".18" stopColor="#e9ecef" /><stop offset=".82" stopColor="#e9ecef" /><stop offset="1" stopColor="#b9c0c6" /></linearGradient>
          <linearGradient id="mapper-marquee" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stopColor="#123038" /><stop offset="1" stopColor="#081418" /></linearGradient>
          <linearGradient id="mapper-glass" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stopColor="#ffffff" stopOpacity=".08" /><stop offset=".45" stopColor="#ffffff" stopOpacity="0" /></linearGradient>
          <radialGradient id="mapper-red" cx=".38" cy=".35" r=".7"><stop offset="0" stopColor="#ff7a80" /><stop offset=".55" stopColor="#fd4851" /><stop offset="1" stopColor="#b8262e" /></radialGradient>
          <radialGradient id="mapper-blue" cx=".38" cy=".35" r=".7"><stop offset="0" stopColor="#7fb0ff" /><stop offset=".55" stopColor="#2f7bff" /><stop offset="1" stopColor="#1846a8" /></radialGradient>
        </defs>
        {/* Corpo de cima (topo bem arredondado), marquise com a logo e o monitor fundo: o desenho do
            gabinete da Coleção. A tela mostra o que fazer. */}
        <path d="M132 8 H508 Q556 8 558 60 L562 436 H78 L82 60 Q84 8 132 8 Z" fill="url(#mapper-body)" stroke="#7d878e" strokeWidth="3" />
        <rect x="116" y="24" width="408" height="44" rx="16" fill="url(#mapper-marquee)" stroke="#57fcff" strokeWidth="2.4" />
        <rect x="120" y="27" width="400" height="38" rx="14" fill="none" stroke="#57fcff" strokeOpacity=".25" strokeWidth="2" />
        <image href="/img/logo.png" x="248" y="29" width="144" height="34" preserveAspectRatio="xMidYMid meet" aria-hidden="true" />
        <path d="M100 84 Q100 76 108 76 H532 Q540 76 540 84 L544 436 H96 Z" fill="#14181b" />
        <rect x="104" y="88" width="432" height="324" rx="24" fill="#050708" stroke="#2a3136" strokeWidth="4" />
        <rect x="116" y="100" width="408" height="306" rx="12" fill="#02090b" />
        <g className={`mapper-screen ${waiting ? "is-waiting" : ""}`} aria-live="polite">
          {screenLines.map((line, index) => (
            <text key={index} x="320" y={index === 0 ? 240 : 284} textAnchor="middle">{line}</text>
          ))}
        </g>
        <rect x="116" y="100" width="408" height="306" rx="12" fill="url(#mapper-glass)" pointerEvents="none" />

        {/* Painel de controle preto, largo e projetado à frente */}
        <path d="M68 436 H572 Q588 436 588 454 L590 518 Q590 536 572 536 H68 Q50 536 50 518 L52 454 Q52 436 68 436 Z" fill="#16191c" stroke="#7d878e" strokeWidth="3" />
        <path d="M60 444 H580" stroke="#ffffff" strokeOpacity=".12" strokeWidth="2" />
        {/* Faixa cinza sob os botões, em arco como no Astro City */}
        <polyline points="330,502 384,488 492,482" fill="none" stroke="#5c656b" strokeWidth="46" strokeLinecap="round" strokeLinejoin="round" />
        <polyline points="330,502 384,488 492,482" fill="none" stroke="#3a4045" strokeWidth="42" strokeLinecap="round" strokeLinejoin="round" />

        {/* Alavanca: base, haste e bola vermelha, que se inclina com as direções apertadas; as setas
            em volta são as direções */}
        <circle cx={LEVER_BASE.x} cy={LEVER_BASE.y} r="22" fill="#0b0d0f" stroke="#2a3136" strokeWidth="2" />
        <circle cx={LEVER_BASE.x} cy={LEVER_BASE.y} r="7" fill="#22272b" />
        <line ref={lever.shaft} x1={LEVER_BASE.x} y1={LEVER_BASE.y} x2={LEVER_BALL.x} y2={LEVER_BALL.y} stroke="#3a4045" strokeWidth="7" strokeLinecap="round" />
        <g ref={lever.ball} className="mapper-lever-ball">
          <circle cx={LEVER_BALL.x} cy={LEVER_BALL.y} r="14" fill="url(#mapper-red)" />
          <circle cx={LEVER_BALL.x - 4} cy={LEVER_BALL.y - 5} r="4" fill="#fff" fillOpacity=".35" />
        </g>
        {(["up", "right", "down", "left"] as ControlId[]).map((id) => (
          <g key={id} {...spotProps(id)}>
            <circle cx={SPOTS[id].x} cy={SPOTS[id].y} r={SPOTS[id].r} className="mapper-hit" />
            <path d="M0 -7 L7 5 H-7 Z" transform={`translate(${SPOTS[id].x} ${SPOTS[id].y}) rotate(${ARROW[id]})`} className="mapper-arrow" />
          </g>
        ))}

        {/* START azul, pequeno, entre a alavanca e os botões */}
        <g {...spotProps("start")}>
          <circle cx={SPOTS.start.x} cy={SPOTS.start.y} r={SPOTS.start.r + 8} className="mapper-hit" />
          <circle cx={SPOTS.start.x + 1} cy={SPOTS.start.y + 2} r={SPOTS.start.r} fill="#0b0d0f" />
          <circle cx={SPOTS.start.x} cy={SPOTS.start.y} r={SPOTS.start.r} fill="url(#mapper-blue)" className="mapper-cap" />
          <text x={SPOTS.start.x} y={SPOTS.start.y + 26} textAnchor="middle" className="mapper-caption">START</text>
        </g>

        {/* 4 botões vermelhos em arco */}
        {(["button1", "button2", "button3", "button4"] as ControlId[]).map((id, index) => (
          <g key={id} {...spotProps(id)}>
            <circle cx={SPOTS[id].x} cy={SPOTS[id].y} r={SPOTS[id].r + 5} className="mapper-hit" />
            <ellipse cx={SPOTS[id].x + 2} cy={SPOTS[id].y + 4} rx={SPOTS[id].r} ry={SPOTS[id].r * 0.8} fill="#0b0d0f" />
            <g className="mapper-cap">
              <circle cx={SPOTS[id].x} cy={SPOTS[id].y} r={SPOTS[id].r} fill="url(#mapper-red)" />
              <text x={SPOTS[id].x} y={SPOTS[id].y + 6} textAnchor="middle" className="mapper-number">{index + 1}</text>
            </g>
          </g>
        ))}

        {/* Corpo de baixo: tampa de manutenção e porta da ficha (clicável) */}
        <path d="M78 536 H562 V634 Q562 648 548 648 H92 Q78 648 78 634 Z" fill="url(#mapper-body)" stroke="#7d878e" strokeWidth="3" />
        <rect x="112" y="550" width="260" height="78" rx="8" fill="none" stroke="#a3abb1" strokeWidth="2.4" />
        <circle cx="352" cy="589" r="5" fill="#8a939a" />
        <g {...spotProps("coin")}>
          <rect x="396" y="548" width="132" height="82" rx="6" className="mapper-hit mapper-door" />
          <rect x="446" y="556" width="32" height="32" rx="4" fill="#9aa3a9" stroke="#7d878e" strokeWidth="2" />
          <rect x="460" y="562" width="4" height="20" rx="2" fill="#2b3035" className="mapper-coin-slot" />
          <rect x="440" y="596" width="44" height="12" rx="3" fill="#8a939a" stroke="#7d878e" />
          <text x="462" y="624" textAnchor="middle" className="mapper-caption is-dark">{t("FICHA")}</text>
        </g>
        <line x1="79.5" y1="638" x2="560.5" y2="638" stroke="#8a939a" strokeWidth="3" />
      </svg>
      </div>

      <div className="cabinet-mapper-side">
        <p className="muted cabinet-mapper-help" role="status">
          {engine === "unavailable"
            ? t("O mapeamento usa o ShmupX Emulator, que não foi encontrado neste computador.")
            : waiting
              ? t("Aperte o botão, tecla ou direção para {control}. Esc cancela.", { control: labelOf(waiting) })
              : flash?.text ?? t("Clique num botão do painel e aperte o botão do seu controle ou uma tecla. Grava na hora.")}
        </p>
        <ul className="cabinet-mapper-list">
          {CONTROLS.map((control) => (
            <li key={control.id}>
              <button
                type="button"
                className={`cabinet-mapper-row ${waiting === control.id ? "is-waiting" : ""} ${hover === control.id ? "is-hover" : ""} ${mapping[control.id] ? "is-mapped" : ""}`}
                disabled={engine !== "ready"}
                onClick={() => pick(control.id)}
                onMouseEnter={() => setHover(control.id)}
                onMouseLeave={() => setHover(null)}
              >
                <span className="cabinet-mapper-name">{t(control.label)}</span>
                <span className="cabinet-mapper-binding">{waiting === control.id ? t("Aperte agora…") : bindingOf(control.id)}</span>
              </button>
            </li>
          ))}
        </ul>
        <div className="cabinet-mapper-actions">
          <button type="button" className="secondary-action" disabled={Object.keys(mapping).length === 0} onClick={() => { setWaiting(null); reset(); }}>{t("Restaurar controles padrão")}</button>
          {!saved && <span className="muted">{t("Não foi possível salvar neste dispositivo.")}</span>}
        </div>
      </div>
    </div>
  );
}
