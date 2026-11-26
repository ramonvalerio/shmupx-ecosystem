import type { ButtonHTMLAttributes } from "react";
import { useI18n } from "../i18n";

/**
 * O X de fechar das modais, no canto superior direito do cabeçalho (o padrão da modal do Monitor).
 * Atributos extras (`data-dialog-close` do Big Picture, `data-tutorial`) vão para o botão.
 */
export default function DialogClose({ onClick, label, className = "", ...rest }: {
  onClick: () => void;
  /** Texto para leitores de tela e dica do mouse; padrão: "Fechar". */
  label?: string;
} & Omit<ButtonHTMLAttributes<HTMLButtonElement>, "onClick" | "type" | "children">) {
  const { t } = useI18n();
  const text = label ?? t("Fechar");
  return (
    <button type="button" className={`dialog-close ${className}`.trim()} aria-label={text} title={text} onClick={onClick} {...rest}>
      <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round">
        <path d="M3 3l10 10M13 3L3 13" />
      </svg>
    </button>
  );
}
