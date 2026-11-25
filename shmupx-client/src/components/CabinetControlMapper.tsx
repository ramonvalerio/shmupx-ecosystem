import { useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n";
import { CONTROLS, describeInput, useControlMapping, type ControlId, type MappedControl } from "../data/emulatorControls";
import { useEngineCapture, type CaptureEvent } from "../data/engineCapture";
import { useCabinetPanel, type LeverGeometry } from "./cabinetPanel";
import { acceptsInput, useConnectedControllers, useControllerChoice, type ControllerChoice } from "../data/connectedControllers";

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

/** Base da alavanca, bola em repouso (um pouco acima, pela perspectiva) e quanto ela anda até o fim. */
const LEVER: LeverGeometry = { base: { x: 186, y: 486 }, ball: { x: 185, y: 482 }, travel: 12 };

/**
 * Gabinete arcade grande para mapear os controles do ShmupX Emulator: clique num botão do painel
 * (ou na lista) e aperte o botão, tecla ou direção que quer usar. Grava na hora, sem "Salvar".
 * A leitura é do próprio motor, escondido, para o código ser o que o jogo usa.
 */
export default function CabinetControlMapper() {
  const { t } = useI18n();
  const { mapping, assign, reset, saved } = useControlMapping();
  const [waiting, setWaiting] = useState<ControlId | null>(null);
  const [flash, setFlash] = useState<{ control: ControlId; text: string } | null>(null);
  const [hover, setHover] = useState<ControlId | null>(null);
  const waitingRef = useRef<ControlId | null>(null);
  waitingRef.current = waiting;
  const { choice, choose } = useControllerChoice();
  const { controllers, seen } = useConnectedControllers(Object.values(mapping).map((item) => item?.deviceName));

  // Motor em modo captura enquanto a tela está aberta (o mesmo do gabinete da Coleção). Esperando
  // um aperto, ele vira o controle escolhido.
  const onInput = (data: CaptureEvent) => {
    const control = waitingRef.current;
    if (data.code?.startsWith("JOYCODE_")) seen(data.deviceName);
    if (!control || !data.code) return;
    // Aperto de outro aparelho que não o escolhido na lista: ignora e continua esperando.
    if (!acceptsInput(choice, data.code, data.deviceName)) return;
    if (data.code === "KEYCODE_ESC") { setWaiting(null); return; }
    const item: MappedControl = { code: data.code, label: data.label ?? data.code, device: data.device, deviceName: data.deviceName };
    const moved = assign(control, item);
    setWaiting(null);
    const name = CONTROLS.find((c) => c.id === control)!.label;
    const text = moved
      ? t("{control}: {input}. Saiu de {other}, que voltou ao padrão.", { control: t(name), input: describeInput(item, t), other: t(CONTROLS.find((c) => c.id === moved)!.label) })
      : t("{control}: {input}", { control: t(name), input: describeInput(item, t) });
    setFlash({ control, text });
  };
  const { engine, held } = useEngineCapture(true, onInput);

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

  const { pressed, lever } = useCabinetPanel(mapping, held, LEVER);

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
        <circle cx={LEVER.base.x} cy={LEVER.base.y} r="22" fill="#0b0d0f" stroke="#2a3136" strokeWidth="2" />
        <circle cx={LEVER.base.x} cy={LEVER.base.y} r="7" fill="#22272b" />
        <line ref={lever.shaft} x1={LEVER.base.x} y1={LEVER.base.y} x2={LEVER.ball.x} y2={LEVER.ball.y} stroke="#3a4045" strokeWidth="7" strokeLinecap="round" />
        <g ref={lever.ball} className="mapper-lever-ball">
          <circle cx={LEVER.ball.x} cy={LEVER.ball.y} r="14" fill="url(#mapper-red)" />
          <circle cx={LEVER.ball.x - 4} cy={LEVER.ball.y - 5} r="4" fill="#fff" fillOpacity=".35" />
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
        <label className="settings-field cabinet-mapper-device">{t("Controle")}
          <select value={choiceValue(choice)} onChange={(event) => choose(parseChoice(event.target.value))}>
            <option value="all">{t("Qualquer controle ou teclado")}</option>
            <option value="keyboard">{t("Teclado")}</option>
            {controllers.map((pad) => <option key={pad.name} value={`pad:${pad.name}`}>{`${pad.name} · ${pad.connected ? t("Conectado") : t("Desconectado")}`}</option>)}
            {choice.kind === "pad" && !controllers.some((pad) => pad.name === choice.name) && <option value={`pad:${choice.name}`}>{`${choice.name} · ${t("Desconectado")}`}</option>}
          </select>
        </label>
        {controllers.length === 0 && <p className="muted">{t("Nenhum controle conectado. Plugue o controle e aperte um botão para ele aparecer na lista.")}</p>}
        {choice.kind === "pad" && !controllers.find((pad) => pad.name === choice.name)?.connected && <p className="mode-error" role="status">{t("{controller} está desconectado. Plugue o controle para mapear os botões.", { controller: choice.name })}</p>}
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

function choiceValue(choice: ControllerChoice): string {
  return choice.kind === "pad" ? `pad:${choice.name}` : choice.kind;
}

function parseChoice(value: string): ControllerChoice {
  if (value === "keyboard") return { kind: "keyboard" };
  if (value.startsWith("pad:")) return { kind: "pad", name: value.slice(4) };
  return { kind: "all" };
}
