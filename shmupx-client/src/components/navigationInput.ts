export type NavigationAction = "left" | "right" | "up" | "down" | "confirm" | "back";
interface PadSnapshot { buttons: readonly { pressed: boolean }[]; axes: readonly number[]; }

/** Neutral arms the pad; only directions repeat, never activation or back. */
export class GamepadNavigationInput {
  private armed = false;
  private previous: NavigationAction | null = null;
  private repeatAt = 0;

  reset() { this.armed = false; this.previous = null; this.repeatAt = 0; }

  read(pad: PadSnapshot, now: number): NavigationAction | null {
    const button = (index: number) => pad.buttons[index]?.pressed === true;
    const x = pad.axes[0] ?? 0;
    const y = pad.axes[1] ?? 0;
    const action: NavigationAction | null = button(1) ? "back" : button(0) ? "confirm" :
      button(14) || x < -0.6 ? "left" : button(15) || x > 0.6 ? "right" :
      button(12) || y < -0.6 ? "up" : button(13) || y > 0.6 ? "down" : null;
    if (!action) { this.armed = true; this.previous = null; return null; }
    if (!this.armed) return null;
    if (action !== this.previous) {
      this.previous = action;
      this.repeatAt = now + 400;
      return action;
    }
    if (action !== "confirm" && action !== "back" && now >= this.repeatAt) {
      this.repeatAt = now + 160;
      return action;
    }
    return null;
  }
}

const FOCUSABLE = "button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),a[href]";
export function visibleControls(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((element) => element.getClientRects().length > 0 && !element.closest("[inert]"));
}
export function visibleDialog(): HTMLElement | undefined {
  return [...document.querySelectorAll<HTMLElement>('[role="dialog"]')].filter((dialog) => dialog.getClientRects().length > 0).at(-1);
}
export function moveFocus(root: HTMLElement, direction: NavigationAction): boolean {
  const controls = visibleControls(root);
  const current = document.activeElement;
  if (!(current instanceof HTMLElement) || !root.contains(current)) { controls[0]?.focus(); return controls.length > 0; }
  const origin = current.getBoundingClientRect();
  const x = origin.x + origin.width / 2;
  const y = origin.y + origin.height / 2;
  const horizontal = direction === "left" || direction === "right";
  const sign = direction === "left" || direction === "up" ? -1 : 1;
  const candidates = controls.filter((element) => element !== current).map((element) => {
    const rect = element.getBoundingClientRect();
    const dx = rect.x + rect.width / 2 - x;
    const dy = rect.y + rect.height / 2 - y;
    return { element, forward: (horizontal ? dx : dy) * sign, lateral: Math.abs(horizontal ? dy : dx) };
  }).filter((candidate) => candidate.forward > 4)
    .sort((a, b) => (a.forward + a.lateral * 3) - (b.forward + b.lateral * 3));
  if (!candidates[0]) return false;
  candidates[0].element.focus();
  candidates[0].element.scrollIntoView({ block: "nearest", inline: "nearest" });
  return true;
}
