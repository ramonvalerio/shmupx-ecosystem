import { useEffect, useRef, useSyncExternalStore } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Evento do motor em modo captura (Rust `steam/controles.rs`). `pid`: o processo do motor que mandou. */
export interface CaptureEvent {
  pid: number;
  kind: "ready" | "input" | "release" | "ended";
  code?: string;
  label?: string;
  device?: string;
  deviceName?: string;
}

export type EngineState = "starting" | "ready" | "unavailable";

interface CaptureSnapshot {
  engine: EngineState;
  /** Códigos apertados agora no controle ou teclado (o motor avisa aperto e soltura). */
  held: ReadonlySet<string>;
}

/**
 * Um motor em modo captura para o Client inteiro: o gabinete de Minhas coleções, a tela de
 * Configurações › Controle e a modal de controles usam o mesmo. Ele abre com o primeiro que precisa
 * e fecha quando o último sai (com uma folga curta, para trocar de tela sem reabrir o motor).
 */
const EMPTY: ReadonlySet<string> = new Set();
const IDLE: CaptureSnapshot = { engine: "starting", held: EMPTY };
const UNAVAILABLE: CaptureSnapshot = { engine: "unavailable", held: EMPTY };
let snapshot: CaptureSnapshot = IDLE;
const listeners = new Set<() => void>();
const inputHandlers = new Set<(event: CaptureEvent) => void>();
let users = 0;
let restarts = 0;
let stopTimer = 0;
/** Motor atual (o PID que o `motor_captura_iniciar` devolveu) e os que já avisaram que estão prontos. */
let currentPid: number | null = null;
const readyPids = new Set<number>();
let listening: Promise<unknown> | null = null;

function update(next: Partial<CaptureSnapshot>) {
  snapshot = { ...snapshot, ...next };
  for (const listener of listeners) listener();
}

function onEvent(data: CaptureEvent) {
  if (data.kind === "ready") readyPids.add(data.pid);
  if (users === 0 || data.pid !== currentPid) return;
  if (data.kind === "ready") { restarts = 0; update({ engine: "ready" }); return; }
  if (data.kind === "ended") {
    // Fechou sozinho (Esc no motor, falha): abre de novo, até 3 vezes seguidas.
    currentPid = null;
    update({ held: EMPTY });
    if (restarts++ < 3) { update({ engine: "starting" }); start(); } else update({ engine: "unavailable" });
    return;
  }
  if (data.kind === "release") {
    if (data.code && snapshot.held.has(data.code)) {
      const next = new Set(snapshot.held);
      next.delete(data.code);
      update({ held: next });
    }
    return;
  }
  if (data.code && !snapshot.held.has(data.code)) update({ held: new Set(snapshot.held).add(data.code) });
  for (const handler of inputHandlers) handler(data);
}

function start() {
  listening ??= listen<CaptureEvent>("engine://capture", (event) => onEvent(event.payload));
  void listening
    .then(() => invoke<number>("motor_captura_iniciar"))
    .then((pid) => {
      // Todo mundo saiu enquanto o motor abria: fecha ele.
      if (users === 0) { void invoke("motor_captura_parar").catch(() => undefined); return; }
      currentPid = pid;
      // O "pronto" pode chegar antes da resposta do comando.
      if (readyPids.has(pid)) { restarts = 0; update({ engine: "ready" }); }
    })
    .catch(() => { if (users > 0) update({ engine: "unavailable" }); });
}

function acquire() {
  window.clearTimeout(stopTimer);
  if (users++ === 0 && currentPid === null) { update({ engine: "starting", held: EMPTY }); restarts = 0; start(); }
}

function release() {
  if (--users > 0) return;
  window.clearTimeout(stopTimer);
  stopTimer = window.setTimeout(() => {
    if (users > 0) return;
    currentPid = null;
    readyPids.clear();
    update({ engine: "starting", held: EMPTY });
    void invoke("motor_captura_parar").catch(() => undefined);
  }, 400);
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

/**
 * Liga o motor de captura enquanto `active` e devolve o estado dele e os códigos apertados.
 * `onInput` recebe cada aperto (para gravar o mapeamento).
 */
export function useEngineCapture(active: boolean, onInput?: (event: CaptureEvent) => void): CaptureSnapshot {
  const current = useSyncExternalStore(subscribe, () => snapshot);
  const handlerRef = useRef(onInput);
  handlerRef.current = onInput;
  const tauri = isTauri();
  useEffect(() => {
    if (!active || !tauri) return;
    const handler = (event: CaptureEvent) => handlerRef.current?.(event);
    inputHandlers.add(handler);
    acquire();
    return () => { inputHandlers.delete(handler); release(); };
  }, [active, tauri]);
  if (!tauri) return UNAVAILABLE;
  return active ? current : IDLE;
}
