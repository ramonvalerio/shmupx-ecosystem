import { useI18n } from "../i18n";
import { useState } from "react";
import type { FormEvent } from "react";
import { MAX_MESSAGE_LENGTH } from "../data/useRoomChat";

interface ChatInputProps {
  /** Pode devolver uma Promise: se ela falhar, o texto continua no campo para reenviar. */
  onSendMessage: (text: string) => void | Promise<void>;
  disabled?: boolean;
  error?: string | null;
}

export default function ChatInput({ onSendMessage, disabled = false, error }: ChatInputProps) {
  const { t } = useI18n();
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmed = draft.trim();
    if (!trimmed || sending) return;
    setSending(true);
    try {
      await onSendMessage(trimmed);
      setDraft("");
    } catch {
      // O erro já aparece abaixo do campo (useRoomChat); o rascunho fica para tentar de novo.
    } finally {
      setSending(false);
    }
  }

  return (
    <form
      onSubmit={handleSubmit}
      className="flex flex-wrap items-center gap-3 border-t border-white/5 bg-zinc-950 px-6 py-4"
    >
      <input
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        placeholder={t("Mensagem para a sala...")}
        maxLength={MAX_MESSAGE_LENGTH}
        disabled={disabled}
        aria-label={t("Mensagem para a sala...")}
        className="flex-1 rounded-md border border-white/10 bg-zinc-900 px-4 py-2 text-sm text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-emerald-400/60"
      />
      <button
        type="submit"
        disabled={disabled || sending || !draft.trim()}
        className="rounded-md border border-emerald-400/60 bg-emerald-400/10 px-4 py-2 text-xs font-semibold uppercase tracking-wider text-emerald-300 transition-colors hover:bg-emerald-400/20"
      >
        {t(" Enviar ")}</button>
      {error && <p className="w-full text-xs text-red-400" role="alert">{t(error)}</p>}
    </form>
  );
}
