import { useI18n } from "../i18n";
import { useEffect, useRef, useState } from "react";
import type { FormEvent, KeyboardEvent } from "react";
import { MAX_MESSAGE_LENGTH } from "../data/useRoomChat";
import { CHAT_EMOJIS, applySuggestion, emojiSuggestions, emojify, emojifyTyping } from "../data/emoji";

interface ChatInputProps {
  /** Pode devolver uma Promise: se ela falhar, o texto continua no campo para reenviar. */
  onSendMessage: (text: string) => void | Promise<void>;
  disabled?: boolean;
  error?: string | null;
  placeholder?: string;
}

/**
 * Campo de mensagem do chat, com emojis: botão com a grade de emojis, sugestões ao digitar `:` e
 * duas letras (Tab, Enter ou clique escolhem; setas mudam; Esc fecha), `:nome:` e emoticons
 * clássicos (`:)`, `<3`, `xD`…) viram emoji ao digitar espaço depois ou ao enviar.
 */
export default function ChatInput({ onSendMessage, disabled = false, error, placeholder }: ChatInputProps) {
  const { t } = useI18n();
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [highlight, setHighlight] = useState(0);
  const [dismissed, setDismissed] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const pickerRef = useRef<HTMLDivElement>(null);
  const label = placeholder ?? t("Mensagem para a sala");
  const suggestions = emojiSuggestions(draft);
  const showSuggestions = suggestions && suggestions.query !== dismissed ? suggestions : null;

  useEffect(() => setHighlight(0), [suggestions?.query]);
  // Clique fora da grade fecha.
  useEffect(() => {
    if (!pickerOpen) return;
    const close = (event: MouseEvent) => { if (!pickerRef.current?.contains(event.target as Node)) setPickerOpen(false); };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [pickerOpen]);

  function update(text: string) {
    setDraft(emojifyTyping(text).slice(0, MAX_MESSAGE_LENGTH));
  }

  function insert(emoji: string) {
    const input = inputRef.current;
    const start = input?.selectionStart ?? draft.length;
    const end = input?.selectionEnd ?? draft.length;
    const next = (draft.slice(0, start) + emoji + draft.slice(end)).slice(0, MAX_MESSAGE_LENGTH);
    setDraft(next);
    requestAnimationFrame(() => {
      input?.focus();
      const caret = Math.min(start + emoji.length, next.length);
      input?.setSelectionRange(caret, caret);
    });
  }

  function choose(emoji: string) {
    if (!suggestions) return;
    setDraft(applySuggestion(draft, suggestions.query, emoji).slice(0, MAX_MESSAGE_LENGTH));
    inputRef.current?.focus();
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (!showSuggestions) return;
    const count = showSuggestions.items.length;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      setHighlight((current) => (current + (event.key === "ArrowDown" ? 1 : count - 1)) % count);
    } else if (event.key === "Tab" || event.key === "Enter") {
      event.preventDefault();
      choose(showSuggestions.items[Math.min(highlight, count - 1)].emoji);
    } else if (event.key === "Escape") {
      event.preventDefault();
      setDismissed(showSuggestions.query);
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmed = emojify(draft.trim());
    if (!trimmed || sending) return;
    setSending(true);
    try {
      await onSendMessage(trimmed);
      setDraft("");
      setPickerOpen(false);
    } catch {
      // O erro já aparece abaixo do campo (useRoomChat); o rascunho fica para tentar de novo.
    } finally {
      setSending(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="chat-composer">
      <div className="chat-emoji-anchor" ref={pickerRef}>
        <button type="button" className="chat-emoji-toggle" aria-label={t("Emojis")} title={t("Emojis (ou digite : e o nome, ex.: :fogo:)")} aria-expanded={pickerOpen} disabled={disabled} onClick={() => setPickerOpen((open) => !open)}>😊</button>
        {pickerOpen && (
          <div className="chat-emoji-picker" role="dialog" aria-label={t("Emojis")}>
            <div className="chat-emoji-grid">
              {CHAT_EMOJIS.map((item) => (
                <button key={item.emoji} type="button" title={`:${item.names[0]}:`} onClick={() => insert(item.emoji)}>{item.emoji}</button>
              ))}
            </div>
            <p className="chat-emoji-hint">{t("Atalhos: :nome: (ex.: :fogo: :gg:) e :) :D ;) :( :P <3 xD")}</p>
          </div>
        )}
      </div>
      {showSuggestions && (
        <ul className="chat-emoji-suggestions" role="listbox" aria-label={t("Sugestões de emoji")}>
          {showSuggestions.items.map((item, index) => (
            <li key={item.name} role="option" aria-selected={index === highlight}>
              <button type="button" onMouseDown={(event) => event.preventDefault()} onClick={() => choose(item.emoji)}>
                <span className="chat-emoji-char">{item.emoji}</span> :{item.name}:
              </button>
            </li>
          ))}
        </ul>
      )}
      <input ref={inputRef} value={draft} onChange={(event) => update(event.target.value)} onKeyDown={onKeyDown} placeholder={label} maxLength={MAX_MESSAGE_LENGTH} disabled={disabled} aria-label={label} />
      <button type="submit" disabled={disabled || sending || !draft.trim()}>{t("Enviar")}</button>
      {error && <p className="chat-composer-error" role="alert">{t(error)}</p>}
    </form>
  );
}
