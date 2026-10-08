import { useState } from "react";
import type { FormEvent } from "react";

interface ChatInputProps {
  onSendMessage: (text: string) => void;
}

export default function ChatInput({ onSendMessage }: ChatInputProps) {
  const [draft, setDraft] = useState("");

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmed = draft.trim();
    if (!trimmed) return;
    onSendMessage(trimmed);
    setDraft("");
  }

  return (
    <form
      onSubmit={handleSubmit}
      className="flex items-center gap-3 border-t border-white/5 bg-zinc-950 px-6 py-4"
    >
      <input
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        placeholder="Mensagem para a sala..."
        className="flex-1 rounded-md border border-white/10 bg-zinc-900 px-4 py-2 text-sm text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-emerald-400/60"
      />
      <button
        type="submit"
        className="rounded-md border border-emerald-400/60 bg-emerald-400/10 px-4 py-2 text-xs font-semibold uppercase tracking-wider text-emerald-300 transition-colors hover:bg-emerald-400/20"
      >
        Enviar
      </button>
    </form>
  );
}
