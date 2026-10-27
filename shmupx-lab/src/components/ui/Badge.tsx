import type { ReactNode } from "react";

type Tone = "neutral" | "success" | "warning" | "info" | "danger";

const TONES: Record<Tone, string> = {
  danger: "bg-red-500/10 text-red-300 ring-red-400/20",
  neutral: "bg-zinc-800/80 text-zinc-400 ring-white/10",
  success: "bg-emerald-500/10 text-emerald-300 ring-emerald-400/20",
  warning: "bg-amber-500/10 text-amber-300 ring-amber-400/20",
  info: "bg-cyan-400/10 text-cyan-300 ring-cyan-400/20",
};

export default function Badge({ tone = "neutral", children }: { tone?: Tone; children: ReactNode }) {
  return (
    <span className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium whitespace-nowrap ring-1 ring-inset ${TONES[tone]}`}>
      {children}
    </span>
  );
}
