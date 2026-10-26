export const CONTROLLER_ACTIONS = ["up", "down", "left", "right", "confirm", "back"] as const;
export type ControllerAction = typeof CONTROLLER_ACTIONS[number];
export type ControllerBinding = { kind: "button"; index: number } | { kind: "axis"; index: number; direction: -1 | 1 };
export interface ControllerMapping { deviceId: string; bindings: Record<ControllerAction, ControllerBinding>; }
export const MAPPINGS_KEY = "shmupx.client.controller-mappings.v1";
export const MAPPINGS_EVENT = "shmupx-controller-mappings-changed";
export function bindingIdentity(binding: ControllerBinding): string { return binding.kind === "button" ? `b${binding.index}` : `a${binding.index}:${binding.direction}`; }
export function validateMapping(value: unknown): ControllerMapping | null {
  if (!value || typeof value !== "object") return null;
  const mapping = value as ControllerMapping;
  if (typeof mapping.deviceId !== "string" || !mapping.deviceId || mapping.deviceId.length > 512 || !mapping.bindings || typeof mapping.bindings !== "object") return null;
  const identities = new Set<string>();
  const bindings = {} as Record<ControllerAction, ControllerBinding>;
  for (const action of CONTROLLER_ACTIONS) {
    const binding = mapping.bindings[action];
    if (!binding || !Number.isInteger(binding.index) || binding.index < 0 || binding.index > 127 || (binding.kind !== "button" && binding.kind !== "axis")) return null;
    if (binding.kind === "axis" && binding.direction !== -1 && binding.direction !== 1) return null;
    const identity = bindingIdentity(binding);
    if (identities.has(identity)) return null;
    identities.add(identity);
    bindings[action] = binding.kind === "button" ? {kind:"button",index:binding.index} : {kind:"axis",index:binding.index,direction:binding.direction};
  }
  return {deviceId:mapping.deviceId,bindings};
}
export function readMappings(): ControllerMapping[] {
  try { const parsed: unknown = JSON.parse(localStorage.getItem(MAPPINGS_KEY) ?? "[]"); return Array.isArray(parsed) ? parsed.map(validateMapping).filter((profile): profile is ControllerMapping => profile !== null).slice(0, 20) : []; }
  catch { return []; }
}
export function saveMapping(mapping: ControllerMapping): boolean {
  const valid = validateMapping(mapping);
  if (!valid) return false;
  try {
    const profiles = readMappings().filter((profile) => profile.deviceId !== valid.deviceId);
    localStorage.setItem(MAPPINGS_KEY, JSON.stringify([valid, ...profiles].slice(0, 20)));
    window.dispatchEvent(new Event(MAPPINGS_EVENT));
    return true;
  } catch { return false; }
}
export function removeMapping(deviceId: string): boolean {
  try { localStorage.setItem(MAPPINGS_KEY, JSON.stringify(readMappings().filter((profile) => profile.deviceId !== deviceId))); window.dispatchEvent(new Event(MAPPINGS_EVENT)); return true; }
  catch { return false; }
}
export function mappedPad(pad: { buttons: readonly {pressed:boolean}[]; axes: readonly number[] }, mapping: ControllerMapping) {
  const buttons = Array.from({length:16},()=>({pressed:false}));
  const slots: Record<ControllerAction,number> = {up:12,down:13,left:14,right:15,confirm:0,back:1};
  for (const action of CONTROLLER_ACTIONS) {
    const binding = mapping.bindings[action];
    buttons[slots[action]].pressed = binding.kind === "button" ? pad.buttons[binding.index]?.pressed === true : (pad.axes[binding.index] ?? 0) * binding.direction > 0.65;
  }
  return { buttons, axes: [0,0] };
}
