import { useI18n } from "../i18n";
import { useState } from "react";
import type { FormEvent } from "react";
import { MAX_MESSAGE_LENGTH } from "../data/useRoomChat";

interface ChatInputProps {
  /** Pode devolver uma Promise: se ela falhar, o texto continua no campo para reenviar. */
  onSendMessage: (text: string) => void | Promise<void>;
  disabled?: boolean;
  error?: string | null;
  placeholder?: string;
}

export default function ChatInput({ onSendMessage, disabled = false, error, placeholder }: ChatInputProps) {
  const { t } = useI18n();
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const label = placeholder ?? t("Mensagem para a sala");

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
    <form onSubmit={handleSubmit} className="chat-composer">
      <input value={draft} onChange={(event) => setDraft(event.target.value)} placeholder={label} maxLength={MAX_MESSAGE_LENGTH} disabled={disabled} aria-label={label} />
      <button type="submit" disabled={disabled || sending || !draft.trim()}>{t("Enviar")}</button>
      {error && <p className="chat-composer-error" role="alert">{t(error)}</p>}
    </form>
  );
}
