import { useCallback, useEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { GamepadNavigationInput, moveFocus, visibleControls, visibleDialog, type NavigationAction } from "./navigationInput";
import { readMappings, mappedPad, MAPPINGS_EVENT } from "./controllerMappings";

/** One shared shell and launch path, with directional keyboard focus in the larger layout. */
export function useBigPicture(startInBigPicture = false) {
  const rootRef = useRef<HTMLElement>(null);
  const previousFocus = useRef<HTMLElement | null>(null);
  const switching = useRef(false);
  const requestedStartup = useRef(startInBigPicture);
  const [active, setActive] = useState(false);
  const [error, setError] = useState("");
  const [controller, setController] = useState("Controle não detectado");
  const [mappings,setMappings] = useState(readMappings);
  useEffect(()=>{
    const refresh=()=>setMappings(readMappings());
    window.addEventListener(MAPPINGS_EVENT,refresh);
    window.addEventListener("storage",refresh);
    return ()=>{window.removeEventListener(MAPPINGS_EVENT,refresh);window.removeEventListener("storage",refresh);};
  },[]);

  const changeMode = useCallback(async (next: boolean) => {
    if (switching.current) return;
    switching.current = true;
    setError("");
    if (next) previousFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    try {
      if (isTauri()) await getCurrentWindow().setFullscreen(next);
      setActive(next);
      requestAnimationFrame(() => {
        if (next) rootRef.current?.querySelector<HTMLButtonElement>(".navigation-tabs button[aria-current]")?.focus();
        else previousFocus.current?.focus();
      });
    } catch { setError("Não foi possível alterar o modo de tela. Tente novamente."); }
    finally { switching.current = false; }
  }, []);

  useEffect(() => {
    if (requestedStartup.current) { requestedStartup.current = false; void changeMode(true); }
  }, [changeMode]);

  useEffect(() => {
    function handleKey(event: KeyboardEvent) {
      if (event.altKey && event.key === "Enter") { event.preventDefault(); void changeMode(!active); return; }
      if (!active) return;
      // A modal owns its focus while open; the shell must not steal arrow keys or Escape.
      if (visibleDialog()) return;
      if (event.key === "Escape") { event.preventDefault(); void changeMode(false); return; }
      if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
      const current = document.activeElement;
      if (!(current instanceof HTMLElement) || !rootRef.current?.contains(current)) return;
      if (current.matches("input,select,textarea,[contenteditable=true]")) return;
      if (moveFocus(rootRef.current, event.key.slice(5).toLowerCase() as NavigationAction)) event.preventDefault();
    }
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [active, changeMode]);

  useEffect(() => {
    if (!active) return;
    const input = new GamepadNavigationInput();
    let frame = 0;
    let padIdentity = "";
    function poll(now: number) {
      frame = requestAnimationFrame(poll);
      if (!document.hasFocus() || document.hidden) { input.reset(); return; }
      let pads: (Gamepad | null)[];
      try { pads = navigator.getGamepads ? [...navigator.getGamepads()] : []; }
      catch { setController("Controle indisponível neste ambiente"); input.reset(); return; }
      const pad = pads.find((candidate) => candidate?.connected && (candidate.mapping === "standard" || mappings.some((mapping)=>mapping.deviceId===candidate.id)));
      if (!pad) { setController("Controle não detectado · use o teclado"); padIdentity = ""; input.reset(); return; }
      const identity = `${pad.index}:${pad.id}`;
      if (identity !== padIdentity) { padIdentity = identity; input.reset(); }
      setController("Controle conectado · direcional / A confirmar / B voltar");
      const scope = visibleDialog() ?? rootRef.current;
      if (scope?.dataset.capturingController === "true") { input.reset(); return; }
      const mapping=mappings.find((profile)=>profile.deviceId===pad.id);
      const action = input.read(mapping ? mappedPad(pad,mapping) : pad, now);
      if (!action || !scope) return;
      if (action === "back") {
        const dialog = visibleDialog();
        if (dialog) dialog.querySelector<HTMLButtonElement>("[data-dialog-close]")?.click();
        else {
          const profileMenu = rootRef.current?.querySelector<HTMLElement>("#profile-actions");
          // O menu do perfil não tem mais botão "Fechar": o "voltar" do controle manda um Esc.
          if (profileMenu) profileMenu.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
          else void changeMode(false);
        }
        return;
      }
      const focused = document.activeElement;
      if (!(focused instanceof HTMLElement) || !scope.contains(focused) || focused.getClientRects().length === 0) { visibleControls(scope)[0]?.focus(); return; }
      if (action === "confirm") {
        if (focused.matches("button:not(:disabled),input[type=checkbox],input[type=radio],a[href]")) focused.click();
      } else if (focused instanceof HTMLSelectElement && (action === "left" || action === "right")) {
        const next = Math.max(0, Math.min(focused.options.length - 1, focused.selectedIndex + (action === "left" ? -1 : 1)));
        if (next !== focused.selectedIndex && !focused.options[next]?.disabled) { focused.selectedIndex = next; focused.dispatchEvent(new Event("change", { bubbles: true })); }
      } else moveFocus(scope, action);
    }
    frame = requestAnimationFrame(poll);
    return () => cancelAnimationFrame(frame);
  }, [active, changeMode, mappings]);

  return { rootRef, active, error, controller, toggle: () => void changeMode(!active) };
}
