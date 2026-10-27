interface ProgressBarProps {
  /** 0 a 100. Ignorado quando `indeterminate`. */
  value: number;
  label: string;
  /** Texto à direita do rótulo (ex.: tempo restante). */
  detail?: string | null;
  /** Andamento desconhecido (etapa sem percentual): barra animada. */
  indeterminate?: boolean;
}

/** Barra de progresso compacta, com rótulo e detalhe acima. */
export default function ProgressBar({ value, label, detail, indeterminate = false }: ProgressBarProps) {
  const percent = Math.max(0, Math.min(100, value));
  return (
    <div className="w-full min-w-0">
      <div className="flex items-baseline justify-between gap-2 text-[11px]">
        <span className="truncate font-medium text-cyan-200">{label}</span>
        {detail && <span className="flex-shrink-0 text-zinc-500 tabular-nums">{detail}</span>}
      </div>
      <div
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={indeterminate ? undefined : Math.round(percent)}
        className="mt-1 h-1.5 overflow-hidden rounded-full bg-white/[0.06]"
      >
        {indeterminate ? (
          <div className="h-full w-1/3 animate-pulse rounded-full bg-cyan-400/60" />
        ) : (
          <div className="h-full rounded-full bg-cyan-400 transition-[width] duration-700 ease-out" style={{ width: `${percent}%` }} />
        )}
      </div>
    </div>
  );
}
