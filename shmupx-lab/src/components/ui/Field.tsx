import type { InputHTMLAttributes, ReactNode, SelectHTMLAttributes } from "react";

/** Aparência comum dos controles de formulário. */
const CONTROL =
  "h-9 w-full rounded-md border border-white/10 bg-zinc-950 px-3 text-sm text-zinc-100 placeholder:text-zinc-600 outline-none transition-colors focus:border-cyan-400/60 focus:ring-2 focus:ring-cyan-400/15 disabled:cursor-not-allowed disabled:text-zinc-500";

interface FieldProps {
  label: string;
  /** Texto de ajuda abaixo do controle. */
  hint?: ReactNode;
  className?: string;
  children: ReactNode;
}

export function Field({ label, hint, className = "", children }: FieldProps) {
  return (
    <label className={`block ${className}`}>
      <span className="mb-1.5 block text-xs font-medium text-zinc-400">{label}</span>
      {children}
      {hint && <span className="mt-1.5 block text-xs leading-relaxed text-zinc-500">{hint}</span>}
    </label>
  );
}

export function TextInput({ className = "", ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={`${CONTROL} ${className}`} {...rest} />;
}

export function Select({ className = "", children, ...rest }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select className={`${CONTROL} ${className}`} {...rest}>
      {children}
    </select>
  );
}
