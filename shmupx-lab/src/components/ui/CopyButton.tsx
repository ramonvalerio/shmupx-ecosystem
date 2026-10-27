import { useEffect, useState, type MouseEvent } from "react";
import { IconCheck, IconCopy } from "./Icons";

interface CopyButtonProps {
  value: string;
  /** Descreve o que é copiado, para leitores de tela (ex.: "Copiar link"). */
  label: string;
}

/** Ícone que copia `value` para a área de transferência e confirma com um check. */
export default function CopyButton({ value, label }: CopyButtonProps) {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), 1500);
    return () => window.clearTimeout(timer);
  }, [copied]);

  async function copy(event: MouseEvent) {
    // Dentro de linhas clicáveis: copiar não abre o painel.
    event.stopPropagation();
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  }

  return (
    <button
      type="button"
      onClick={(event) => void copy(event)}
      title={copied ? "Copiado" : label}
      aria-label={copied ? "Copiado" : label}
      className={`inline-flex h-6 w-6 flex-shrink-0 items-center justify-center rounded transition-colors focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none ${
        copied ? "text-emerald-300" : "text-zinc-500 hover:bg-white/5 hover:text-zinc-200"
      }`}
    >
      {copied ? <IconCheck width={14} height={14} /> : <IconCopy width={14} height={14} />}
    </button>
  );
}
