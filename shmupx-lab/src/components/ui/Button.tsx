import type { ButtonHTMLAttributes } from "react";

type Variant = "primary" | "secondary" | "ghost" | "danger" | "info" | "success";
type Size = "sm" | "md";

const VARIANTS: Record<Variant, string> = {
  primary: "bg-cyan-400 text-zinc-950 hover:bg-cyan-300",
  secondary: "border border-white/10 bg-zinc-900 text-zinc-200 hover:border-white/20 hover:bg-zinc-800",
  ghost: "text-zinc-400 hover:bg-white/5 hover:text-zinc-100",
  // ações destrutivas (remover)
  danger: "bg-red-500 text-white hover:bg-red-400",
  // interromper/desistir (cancelar)
  info: "bg-blue-500 text-white hover:bg-blue-400",
  // confirmar/gravar (salvar)
  success: "bg-emerald-500 text-zinc-950 hover:bg-emerald-400",
};

const SIZES: Record<Size, string> = {
  sm: "h-8 px-3 text-xs",
  md: "h-9 px-4 text-sm",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  /** Mostra um indicador e bloqueia cliques enquanto a ação roda. */
  loading?: boolean;
}

export default function Button({
  variant = "secondary",
  size = "md",
  loading = false,
  disabled,
  className = "",
  type = "button",
  children,
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      className={`inline-flex flex-shrink-0 items-center justify-center gap-2 rounded-md font-medium whitespace-nowrap transition-colors focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50 ${VARIANTS[variant]} ${SIZES[size]} ${className}`}
      {...rest}
    >
      {loading && <span aria-hidden className="h-3.5 w-3.5 animate-spin rounded-full border-2 border-current border-t-transparent" />}
      {children}
    </button>
  );
}
