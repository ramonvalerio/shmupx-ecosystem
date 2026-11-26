/**
 * Interruptor liga/desliga (role="switch"), com o estado escrito ao lado (ON/OFF). Desabilitado,
 * vale como desligado quando `offWhenDisabled`.
 */
export default function ToggleSwitch({ on, onChange, disabled = false, offWhenDisabled = false, labelledBy, title }: {
  on: boolean;
  onChange: (next: boolean) => void;
  disabled?: boolean;
  offWhenDisabled?: boolean;
  labelledBy?: string;
  title?: string;
}) {
  const shown = disabled && offWhenDisabled ? false : on;
  return (
    <button type="button" role="switch" aria-checked={shown} aria-labelledby={labelledBy} disabled={disabled} title={title}
      className={`toggle-switch${shown ? " is-on" : ""}`} onClick={() => onChange(!shown)}>
      <span className="toggle-switch-track" aria-hidden="true"><span className="toggle-switch-knob" /></span>
      <span className="toggle-switch-state" aria-hidden="true">{shown ? "ON" : "OFF"}</span>
    </button>
  );
}
