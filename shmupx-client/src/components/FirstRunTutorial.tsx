import { useCallback, useEffect, useLayoutEffect, useRef, useState, type SyntheticEvent } from "react";
import { createPortal } from "react-dom";
import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useI18n } from "../i18n";
import type { GameRoom } from "../types/room";
import { usePlatformOwnership } from "../data/platforms";
import { useClientMisterGame } from "../data/playing";

/**
 * Tutorial da primeira vez (passos 4 e 5 do site: importar os jogos e jogar). No estilo dos jogos de
 * celular: escurece a tela, abre um recorte em volta do botão certo, uma mão aponta onde tocar e só
 * o recorte recebe o clique. Cada passo avança sozinho quando o jogador chega lá (a janela abriu, os
 * jogos foram importados, o jogo abriu). Os alvos são marcados com `data-tutorial="…"` nas telas.
 */

const tutorialKey = (uid: string) => `shmupx.client.tutorial.v1.${uid}`;
export type TutorialStatus = "done" | "skipped";

export function readTutorialStatus(uid: string): TutorialStatus | null {
  try {
    const saved = JSON.parse(localStorage.getItem(tutorialKey(uid)) ?? "null") as { status?: unknown } | null;
    return saved?.status === "done" || saved?.status === "skipped" ? saved.status : null;
  } catch { return null; }
}
function saveTutorialStatus(uid: string, status: TutorialStatus) {
  try { localStorage.setItem(tutorialKey(uid), JSON.stringify({ status, at: Date.now() })); } catch { /* vale só nesta sessão */ }
}
/** Já jogou neste PC (escolheu modo/plataforma de algum jogo): não é a primeira vez. */
function playedBefore() {
  try { return Object.keys(JSON.parse(localStorage.getItem("shmupx.client.roomPlay.v1") ?? "{}") as object).length > 0; } catch { return false; }
}

/**
 * Abre o tutorial uma vez, depois do primeiro login (catálogo carregado). Quem já tem biblioteca e
 * já jogou neste PC não vê sozinho; "Rever tutorial" (Configurações › Interface) abre de novo.
 */
export function useFirstRunTutorial(uid: string, rooms: GameRoom[]) {
  const playable = usePlatformOwnership();
  const [active, setActive] = useState(false);
  const loaded = rooms.length > 0;
  useEffect(() => {
    if (!loaded || readTutorialStatus(uid) || (playable.anyLibrary && playedBefore())) return;
    const timer = window.setTimeout(() => setActive(true), 700);
    return () => window.clearTimeout(timer);
    // Decide uma vez por login, quando o catálogo chega.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [uid, loaded]);
  const start = useCallback(() => setActive(true), []);
  const finish = useCallback((status: TutorialStatus) => { saveTutorialStatus(uid, status); setActive(false); }, [uid]);
  return { active, start, finish };
}

type StepId =
  | "import-open" | "go-collection" | "import-source" | "import-run" | "import-scanning" | "import-folder" | "import-other" | "import-close"
  | "pick-game" | "pick-mode" | "insert-coin" | "choose-mode" | "launch-problem" | "launching" | "done";

interface StepInfo {
  code: string;
  /** 1 = importar jogos, 2 = jogar e rankear. */
  phase: 1 | 2;
  /** Posição dentro do passo, para os marcadores. */
  index: number;
  title: string;
  text: string;
  /** `data-tutorial` do alvo; sem alvo, o balão fica no centro. */
  target: string | null;
  /** A mão aponta (alvos grandes, como uma janela inteira, ficam só com o recorte). */
  hand: boolean;
  /** Bloqueia o resto da tela (sem alvo: só o "Concluir" da tela final). */
  block: boolean;
  next?: boolean;
  /** Onde a mão aponta dentro do alvo (seletor CSS); sem isso, no meio dele. */
  point?: string;
}

const PHASE_SIZE = { 1: 4, 2: 5 } as const;
const PHASE_LABEL = { 1: "Passo 1 · Importar jogos", 2: "Passo 2 · Jogar e rankear" } as const;

function stepInfo(id: StepId, importing: boolean): StepInfo {
  switch (id) {
    case "import-open": return { code: "1.1", phase: 1, index: 1, target: "import-button", hand: true, block: true,
      title: "Importe seus jogos", text: "Toque em Importar jogos para trazer os jogos que você já tem neste computador." };
    case "go-collection": return { code: importing ? "1.1" : "2.1", phase: importing ? 1 : 2, index: 1, target: "crumb-collection", hand: true, block: true,
      title: "Volte para a coleção", text: "Toque em Minhas coleções para ver os seus jogos." };
    case "import-source": return { code: "1.2", phase: 1, index: 2, target: "import-sources", hand: true, block: true, point: ".import-source-row:nth-child(2) .import-source-option",
      title: "De onde vêm os seus jogos?", text: "Steam lê os jogos instalados, Emulador lê os seus ROMs (.zip) e MiSTer lê os jogos do MiSTer na rede. Toque numa opção." };
    case "import-run": return { code: "1.3", phase: 1, index: 3, target: "import-run", hand: true, block: true,
      title: "Leia a sua biblioteca", text: "Toque aqui. O ShmupX procura os jogos sozinho e marca na coleção os que você tem." };
    case "import-scanning": return { code: "1.3", phase: 1, index: 3, target: "import-dialog", hand: false, block: true,
      title: "Procurando os seus jogos…", text: "Aguarde um instante enquanto o ShmupX lê a biblioteca." };
    case "import-folder": return { code: "1.3", phase: 1, index: 3, target: "import-pick-folder", hand: true, block: true,
      title: "Mostre onde estão os seus jogos", text: "Não achamos os ROMs sozinhos. Toque em Selecionar pasta de ROMs e escolha a pasta com os arquivos .zip (por exemplo, a pasta de ROMs do Fightcade)." };
    case "import-other": return { code: "1.3", phase: 1, index: 3, target: "import-back", hand: true, block: true,
      title: "Tente outra fonte", text: "Aqui não há jogos do ShmupX neste computador. Toque em Voltar e escolha outra fonte." };
    case "import-close": return { code: "1.4", phase: 1, index: 4, target: "import-close", hand: true, block: true,
      title: "Jogos importados!", text: "A coleção já mostra os jogos que você tem. Toque em Fechar para voltar a ela." };
    case "pick-game": return { code: "2.1", phase: 2, index: 1, target: "game-owned", hand: true, block: true,
      title: "Escolha um jogo", text: "Toque num jogo colorido: são os que você tem. Os detalhes e a ficha para jogar aparecem ao lado." };
    case "pick-mode": return { code: "2.2", phase: 2, index: 2, target: "play-modes", hand: true, block: true, next: true,
      title: "Escolha o modo de pontuação", text: "Hi-Score: pontuação máxima, com quantos créditos quiser. 1CC: zerar com 1 crédito. Caravan: pontuação em tempo limitado. Toque num modo ou em Próximo." };
    case "insert-coin": return { code: "2.3", phase: 2, index: 3, target: "play-coin", hand: true, block: true,
      title: "Insira a ficha", text: "Toque na ficha para jogar." };
    case "choose-mode": return { code: "2.4", phase: 2, index: 4, target: "play-mode-choices", hand: true, block: true, point: ".play-mode-choice.is-training",
      title: "Como você quer jogar?", text: "Training Mode é grátis e não vale ranking: ótimo para começar. Ranked Mode gasta 1 ficha, grava a run e vale para o ranking." };
    case "launch-problem": return { code: "2.5", phase: 2, index: 5, target: "launch-problem", hand: false, block: true,
      title: "O jogo não abriu", text: "Siga as dicas da janela. Se os ROMs estão em outra pasta, toque em Selecionar pasta de ROMs e depois tente de novo." };
    case "launching": return { code: "2.5", phase: 2, index: 5, target: null, hand: false, block: false,
      title: "Abrindo o jogo…", text: "Quando a janela do jogo aparecer, é só jogar!" };
    case "done": return { code: "✓", phase: 2, index: 5, target: null, hand: false, block: true,
      title: "Tudo pronto!", text: "Você já sabe jogar no ShmupX. Faça o seu recorde e suba no ranking. Para ver este tutorial de novo, toque em HOW TO USE, no canto superior direito." };
  }
}

/** Primeiro alvo visível com o nome (as telas escondidas com `hidden` ficam sem tamanho). */
function findTarget(name: string): HTMLElement | null {
  for (const element of document.querySelectorAll<HTMLElement>(`[data-tutorial="${name}"]`)) {
    if (element.closest(".tut-root")) continue;
    const rect = element.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) return element;
  }
  return null;
}

interface Box { x: number; y: number; w: number; h: number }

/**
 * Área do alvo na tela: o próprio elemento somado aos filhos que transbordam dele (os cartões da janela
 * da ficha passam da caixa), cortada pelas caixas com rolagem em volta (itens fora da lista rolável não contam).
 */
function targetBox(element: HTMLElement): Box {
  const r = element.getBoundingClientRect();
  let left = r.left, top = r.top, right = r.right, bottom = r.bottom;
  const children = element.querySelectorAll<HTMLElement>("*");
  for (let i = 0; i < children.length && i < 300; i++) {
    const c = children[i].getBoundingClientRect();
    if (c.width === 0 || c.height === 0) continue;
    left = Math.min(left, c.left); top = Math.min(top, c.top); right = Math.max(right, c.right); bottom = Math.max(bottom, c.bottom);
  }
  // Janelas (position: fixed) não são cortadas pelas caixas com rolagem de fora delas.
  for (let node: HTMLElement = element, parent = element.parentElement; parent && parent !== document.body; node = parent, parent = parent.parentElement) {
    if (getComputedStyle(node).position === "fixed") break;
    const style = getComputedStyle(parent);
    if (style.overflowX === "visible" && style.overflowY === "visible") continue;
    const p = parent.getBoundingClientRect();
    left = Math.max(left, p.left); top = Math.max(top, p.top); right = Math.min(right, p.right); bottom = Math.min(bottom, p.bottom);
  }
  return { x: Math.round(left), y: Math.round(top), w: Math.max(0, Math.round(right - left)), h: Math.max(0, Math.round(bottom - top)) };
}
interface Snapshot { step: StepId; rect: Box | null; point: Box | null; missing: boolean; vw: number; vh: number }
const sameSnapshot = (a: Snapshot, b: Snapshot) => JSON.stringify(a) === JSON.stringify(b);

const HOLE_PAD = 6;
const LAUNCH_SLOW_MS = 25_000;

export default function FirstRunTutorial({ rooms, currentRoomId, paused = false, onExit }: {
  rooms: GameRoom[];
  /** Jogo na tela (sala aberta ou o selecionado na Coleção): a ficha dele joga. */
  currentRoomId: string | null;
  /** Big Picture: o tutorial espera. */
  paused?: boolean;
  onExit: (status: TutorialStatus) => void;
}) {
  const { t } = useI18n();
  const playable = usePlatformOwnership();
  const owns = (room: GameRoom) => playable.tags(room).some((tag) => tag.owned);
  const ownedCount = rooms.filter(owns).length;
  const currentRoom = rooms.find((room) => room.id === currentRoomId);
  const currentOwned = currentRoom ? owns(currentRoom) : false;

  const [modeConfirmed, setModeConfirmed] = useState(false);
  /** Passou pela importação (a janela leu uma biblioteca com jogos do ShmupX): libera o passo 2. */
  const [imported, setImported] = useState(false);
  const [launchRequested, setLaunchRequested] = useState<number | null>(null);
  const [launched, setLaunched] = useState(false);
  const [launchSlow, setLaunchSlow] = useState(false);
  const [nudge, setNudge] = useState(0);

  // Jogo aberto (Steam, ShmupX Emulator ou MiSTer, por qualquer botão): o tutorial termina.
  useEffect(() => {
    if (!isTauri()) return;
    const stops = [
      listen("steam://game-launched", () => setLaunched(true)),
      listen("emulator://game-launched", () => setLaunched(true)),
    ];
    return () => { stops.forEach((stop) => void stop.then((unlisten) => unlisten())); };
  }, []);
  const misterGame = useClientMisterGame();
  const misterAtStart = useRef(misterGame);
  useEffect(() => { if (misterGame && misterGame !== misterAtStart.current) setLaunched(true); }, [misterGame]);

  // Cliques que contam como "chegou lá": escolheu um modo, ou Training/Ranked na janela da ficha.
  useEffect(() => {
    function onClick(event: MouseEvent) {
      const target = event.target instanceof Element ? event.target : null;
      if (!target || target.closest(".tut-root")) return;
      if (target.closest('[data-tutorial="play-modes"] button')) setModeConfirmed(true);
      const choice = target.closest<HTMLButtonElement>('[data-tutorial="play-mode-choices"] button');
      if (choice && !choice.disabled) setLaunchRequested(Date.now());
    }
    function onKey(event: KeyboardEvent) {
      if ((event.key === "1" || event.key === "2") && findTarget("play-mode-choices")) setLaunchRequested(Date.now());
    }
    document.addEventListener("click", onClick, true);
    document.addEventListener("keydown", onKey, true);
    return () => { document.removeEventListener("click", onClick, true); document.removeEventListener("keydown", onKey, true); };
  }, []);
  useEffect(() => {
    setLaunchSlow(false);
    if (launchRequested === null) return;
    const timer = window.setTimeout(() => setLaunchSlow(true), LAUNCH_SLOW_MS);
    return () => window.clearTimeout(timer);
  }, [launchRequested]);

  /** Em qual passo o jogador está, pelo que está na tela agora. */
  const resolve = useCallback((): StepId => {
    if (launched) return "done";
    if (findTarget("launch-problem")) return "launch-problem";
    if (findTarget("play-mode-choices")) return "choose-mode";
    const dialog = findTarget("import-dialog");
    if (dialog) {
      if (!dialog.dataset.tutorialSource) return "import-source";
      if (findTarget("import-pick-folder")) return "import-folder";
      if (findTarget("import-not-found")) return "import-other";
      if (findTarget("import-scanning")) return "import-scanning";
      if (findTarget("import-run")) return "import-run";
      // Fonte escolhida e leitura concluída: com jogos do ShmupX, fecha; sem, tenta outra fonte.
      return ownedCount > 0 ? "import-close" : "import-other";
    }
    if (launchRequested !== null) return "launching";
    // Sempre começa pela importação, mesmo para quem já tem jogos na coleção.
    if (!imported || ownedCount === 0) return findTarget("import-button") ? "import-open" : "go-collection";
    if (currentOwned && findTarget("play-coin")) return !modeConfirmed && findTarget("play-modes") ? "pick-mode" : "insert-coin";
    if (!findTarget("game-owned") && findTarget("crumb-collection") && !findTarget("import-button")) return "go-collection";
    return "pick-game";
  }, [launched, ownedCount, launchRequested, currentOwned, modeConfirmed, imported]);

  const [snap, setSnap] = useState<Snapshot | null>(null);
  const missingSince = useRef<{ step: StepId; at: number } | null>(null);
  const measure = useCallback(() => {
    const step = resolve();
    const info = stepInfo(step, !imported || ownedCount === 0);
    const element = info.target ? findTarget(info.target) : null;
    let rect: Box | null = null;
    let point: Box | null = null;
    if (element) {
      rect = targetBox(element);
      const spot = info.point ? element.querySelector<HTMLElement>(info.point) : null;
      if (spot) { const r = spot.getBoundingClientRect(); point = { x: Math.round(r.left), y: Math.round(r.top), w: Math.round(r.width), h: Math.round(r.height) }; }
      missingSince.current = null;
    } else if (info.target && missingSince.current?.step !== step) missingSince.current = { step, at: Date.now() };
    const missing = Boolean(info.target && !element && missingSince.current && Date.now() - missingSince.current.at > 1500);
    const next: Snapshot = { step, rect, point, missing, vw: window.innerWidth, vh: window.innerHeight };
    setSnap((current) => (current && sameSnapshot(current, next) ? current : next));
  }, [resolve, ownedCount, imported]);

  // Chegou ao "Jogos importados!": a importação está feita e o passo 2 fica liberado.
  useEffect(() => { if (snap?.step === "import-close") setImported(true); }, [snap?.step]);

  // Alvos que aparecem depois (janelas, painel de detalhes), redimensionar e rolar: mede de novo.
  useEffect(() => {
    if (paused) return;
    // Junta as mudanças seguidas numa medida só (timer, e não requestAnimationFrame, que para em janela escondida).
    let pending = 0;
    const schedule = () => { if (!pending) pending = window.setTimeout(() => { pending = 0; measure(); }, 16); };
    schedule();
    const observer = new MutationObserver(schedule);
    observer.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ["class", "hidden", "disabled", "aria-checked", "aria-pressed", "data-tutorial-source", "style"] });
    window.addEventListener("resize", schedule);
    window.addEventListener("scroll", schedule, true);
    // Animações (a ficha girando, janelas abrindo) não mudam o DOM: confere a posição de tempos em tempos.
    const interval = window.setInterval(schedule, 250);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", schedule);
      window.removeEventListener("scroll", schedule, true);
      window.clearInterval(interval);
      window.clearTimeout(pending);
    };
  }, [measure, paused]);

  // Passo novo: traz o alvo para a tela (um jogo lá embaixo na lista, por exemplo).
  const step = snap?.step;
  useEffect(() => {
    if (!step) return;
    const name = stepInfo(step, ownedCount === 0).target;
    const element = name ? findTarget(name) : null;
    element?.scrollIntoView?.({ block: "nearest", inline: "nearest" });
    // Problema ao abrir: depois de resolver, o jogador insere a ficha de novo.
    if (step === "launch-problem") setLaunchRequested(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step]);

  const bubbleRef = useRef<HTMLDivElement>(null);
  const [bubbleSize, setBubbleSize] = useState({ w: 360, h: 200 });
  useLayoutEffect(() => {
    const element = bubbleRef.current;
    if (!element) return;
    const w = Math.round(element.offsetWidth), h = Math.round(element.offsetHeight);
    if (w !== bubbleSize.w || h !== bubbleSize.h) setBubbleSize({ w, h });
  });

  if (paused || !snap) return null;
  const info = stepInfo(snap.step, ownedCount === 0);
  const { vw, vh } = snap;
  const hole = snap.rect && !snap.missing ? clampBox({ x: snap.rect.x - HOLE_PAD, y: snap.rect.y - HOLE_PAD, w: snap.rect.w + HOLE_PAD * 2, h: snap.rect.h + HOLE_PAD * 2 }, vw, vh) : null;
  const blocking = info.block && (hole !== null || info.target === null);
  // A ponta do dedo: no meio do alvo (um pouco abaixo, em botões pequenos). Perto do pé da tela, a mão vem de cima.
  const aim = hole ? snap.point ?? hole : null;
  // Alvos altos (cartões): a mão toca acima do meio, sem cobrir o nome escrito embaixo.
  const tip = aim ? { x: aim.x + aim.w / 2, y: aim.y + (aim.h > 120 ? aim.h * 0.36 : Math.min(aim.h * 0.62, aim.h / 2 + 14)) } : null;
  const handDown = tip ? tip.y + 120 > vh : false;
  const showHand = info.hand && tip !== null;
  const bubble = placeBubble(hole, tip, handDown && showHand, showHand, bubbleSize, vw, vh);
  const block = (event: SyntheticEvent) => { event.preventDefault(); event.stopPropagation(); setNudge((n) => n + 1); };
  const finishNow = () => onExit(snap.step === "done" ? "done" : "skipped");

  return createPortal(
    <div className={`tut-root ${blocking ? "is-blocking" : "is-passive"}`} role="dialog" aria-modal={blocking} aria-labelledby="tut-title" aria-describedby="tut-text">
      {blocking && (hole ? <>
        <div className="tut-dim" style={{ left: 0, top: 0, width: vw, height: hole.y }} onPointerDown={block} onClick={block} />
        <div className="tut-dim" style={{ left: 0, top: hole.y + hole.h, width: vw, height: Math.max(0, vh - hole.y - hole.h) }} onPointerDown={block} onClick={block} />
        <div className="tut-dim" style={{ left: 0, top: hole.y, width: hole.x, height: hole.h }} onPointerDown={block} onClick={block} />
        <div className="tut-dim" style={{ left: hole.x + hole.w, top: hole.y, width: Math.max(0, vw - hole.x - hole.w), height: hole.h }} onPointerDown={block} onClick={block} />
      </> : <div className="tut-dim" style={{ left: 0, top: 0, width: vw, height: vh }} onPointerDown={(event) => event.stopPropagation()} />)}
      {hole && <div className="tut-frame" style={{ left: hole.x, top: hole.y, width: hole.w, height: hole.h }} aria-hidden="true" />}
      {showHand && tip && (
        <div key={nudge} className={`tut-hand ${handDown ? "is-down" : ""} ${nudge > 0 ? "is-nudge" : ""}`} style={{ left: tip.x, top: tip.y }} aria-hidden="true">
          <span className="tut-ring" />
          <span className="tut-hand-inner"><HandIcon /></span>
        </div>
      )}
      <div ref={bubbleRef} className={`tut-bubble ${hole ? "" : "is-center"} ${snap.step === "done" ? "is-done" : ""}`} style={{ left: bubble.x, top: bubble.y }}>
        <header className="tut-head">
          <span className="tut-plate">{info.code}</span>
          <span className="tut-kicker"><i aria-hidden="true" />{snap.step === "done" ? t("Tutorial concluído") : t(PHASE_LABEL[info.phase])}</span>
          {snap.step !== "done" && <span className="tut-count">{t("{step} de {total}", { step: info.index, total: PHASE_SIZE[info.phase] })}</span>}
        </header>
        <h2 id="tut-title" className="tut-title">{t(info.title)}</h2>
        <p id="tut-text" className="tut-text">{t(info.text)}</p>
        {snap.missing && <p className="tut-note" role="status">{t("Não encontramos este item na tela agora. Continue pela tela normalmente ou pule o tutorial.")}</p>}
        {snap.step === "launching" && launchSlow && <p className="tut-note" role="status">{t("Se o jogo já abriu, toque em Concluir.")}</p>}
        <footer className="tut-foot">
          {snap.step !== "done" && <button type="button" className="tut-skip" onClick={() => onExit("skipped")}>{t("Pular tutorial")}</button>}
          <span className="tut-pips" aria-hidden="true">
            {Array.from({ length: PHASE_SIZE[info.phase] }, (_, i) => <i key={i} className={i + 1 < info.index || snap.step === "done" ? "is-past" : i + 1 === info.index ? "is-now" : ""} />)}
          </span>
          {info.next && <button type="button" className="tut-next" onClick={() => setModeConfirmed(true)}>{t("Próximo")}</button>}
          {(snap.step === "done" || (snap.step === "launching" && launchSlow)) && <button type="button" className="tut-next" onClick={() => onExit("done")}>{t("Concluir")}</button>}
          {snap.step === "launching" && !launchSlow && <button type="button" className="tut-skip is-quiet" onClick={() => setLaunchRequested(null)}>{t("Voltar")}</button>}
        </footer>
        <button type="button" className="tut-close" aria-label={t("Fechar tutorial")} title={t("Fechar tutorial")} onClick={finishNow}>×</button>
      </div>
    </div>,
    document.body,
  );
}

function clampBox(box: Box, vw: number, vh: number): Box {
  const x = Math.max(0, box.x), y = Math.max(0, box.y);
  return { x, y, w: Math.max(0, Math.min(vw, box.x + box.w) - x), h: Math.max(0, Math.min(vh, box.y + box.h) - y) };
}

/** Balão perto do alvo sem cobrir a mão: abaixo, acima, à direita ou à esquerda; sem espaço, no centro. */
function placeBubble(hole: Box | null, tip: { x: number; y: number } | null, handDown: boolean, hand: boolean, size: { w: number; h: number }, vw: number, vh: number) {
  const m = 16, gap = 14;
  const center = { x: Math.round((vw - size.w) / 2), y: Math.round((vh - size.h) / 2) };
  if (!hole || !tip) return center;
  const clampX = (x: number) => Math.round(Math.min(Math.max(m, x), vw - size.w - m));
  const clampY = (y: number) => Math.round(Math.min(Math.max(m, y), vh - size.h - m));
  const handBelow = hand && !handDown ? tip.y + 112 : hole.y + hole.h;
  const handAbove = hand && handDown ? tip.y - 112 : hole.y;
  const candidates = [
    { x: clampX(tip.x - 48), y: Math.max(hole.y + hole.h, handBelow) + gap, fits: (y: number) => y + size.h <= vh - m },
    { x: clampX(tip.x - 48), y: Math.min(hole.y, handAbove) - gap - size.h, fits: (y: number) => y >= m },
    { x: hole.x + hole.w + gap + (hand ? 80 : 0), y: clampY(tip.y - size.h / 2), fits: (_: number, x: number) => x + size.w <= vw - m },
    { x: hole.x - gap - size.w, y: clampY(tip.y - size.h / 2), fits: (_: number, x: number) => x >= m },
  ];
  for (const c of candidates) if (c.fits(c.y, c.x)) return { x: Math.round(c.x), y: Math.round(c.y) };
  return { x: vw - size.w - m, y: vh - size.h - m };
}

/** Mão de luva apontando (traço chapado, sem brilho): a ponta do indicador fica em (13, 2) da arte, (16, 3) na tela. */
function HandIcon() {
  return (
    <svg className="tut-hand-svg" viewBox="0 0 64 80" width="80" height="100">
      <path className="tut-glove" d="M6 9a7 7 0 0 1 14 0V27a6 6 0 0 1 12 0v2a6 6 0 0 1 12 0v2a6 6 0 0 1 12 0V52c0 9.4-7.6 17-17 17H27c-5.5 0-10.5-2.7-13.6-7.2L2.4 45.6c-1.9-2.8-1.4-6.4 1.1-8 2.5-1.6 5.8-.9 7.6 1.6L6 43Z" />
      <path className="tut-crease" d="M26 33v7M38 35v7M50 37v6" />
      <rect className="tut-cuff" x="19" y="64" width="36" height="14" />
      <path className="tut-cuff-line" d="M19 69h36" />
    </svg>
  );
}
