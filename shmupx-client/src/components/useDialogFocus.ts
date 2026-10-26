import { useEffect, useEffectEvent, useRef } from "react";
import { visibleControls, visibleDialog } from "./navigationInput";

/** Focus stays in the top visible dialog and returns to its trigger when it closes. */
export function useDialogFocus(open: boolean, onClose: () => void) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const close = useEffectEvent(onClose);
  useEffect(() => {
    const dialog = dialogRef.current;
    if (!open || !dialog) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    (visibleControls(dialog)[0] ?? dialog).focus();
    function handleKey(event: KeyboardEvent) {
      if (!dialog || visibleDialog() !== dialog) return;
      if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); close(); }
      if (event.key !== "Tab") return;
      const controls = visibleControls(dialog);
      const first = controls[0];
      const last = controls.at(-1);
      if (!first) { event.preventDefault(); dialog.focus(); return; }
      if (event.shiftKey && (document.activeElement === first || !dialog.contains(document.activeElement))) { event.preventDefault(); last?.focus(); }
      else if (!event.shiftKey && (document.activeElement === last || !dialog.contains(document.activeElement))) { event.preventDefault(); first.focus(); }
    }
    function keepFocus(event: FocusEvent) {
      if (dialog && visibleDialog() === dialog && event.target instanceof Node && !dialog.contains(event.target)) (visibleControls(dialog)[0] ?? dialog).focus();
    }
    document.addEventListener("keydown", handleKey, true);
    document.addEventListener("focusin", keepFocus);
    return () => {
      document.removeEventListener("keydown", handleKey, true);
      document.removeEventListener("focusin", keepFocus);
      if (previous?.isConnected && previous.getClientRects().length > 0) previous.focus();
    };
  }, [open]);
  return dialogRef;
}
