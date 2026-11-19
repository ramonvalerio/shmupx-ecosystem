import { useEffect, useRef } from "react";
import { useI18n } from "../i18n";
import type { ChatMessage } from "../types/room";
import type { ChatStatus } from "../data/useRoomChat";

interface ChatFeedProps {
  messages: ChatMessage[];
  status?: ChatStatus;
  error?: string | null;
  /** uid do jogador conectado: as mensagens dele aparecem destacadas. */
  playerId?: string;
}

export default function ChatFeed({ messages, status = "ready", error, playerId }: ChatFeedProps) {
  const { t } = useI18n();
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [messages]);

  return (
    <div className="scrollbar-arcade flex-1 space-y-3 overflow-y-auto px-6 py-4" aria-live="polite" aria-busy={status === "loading"}>
      {status === "loading" && <p className="empty-state">{t("Carregando o chat da sala…")}</p>}
      {status === "error" && <p className="empty-state" role="alert">{t(error ?? "Não foi possível conectar ao chat da sala.")}</p>}
      {status === "ready" && messages.length === 0 && <p className="empty-state">{t("Nenhuma mensagem ainda. Diga oi para a sala!")}</p>}
      {messages.map((message) => (
        <div key={message.id} className="flex items-start gap-3" style={message.pending ? { opacity: 0.6 } : undefined}>
          <span className="font-arcade shrink-0 pt-0.5 text-[9px] text-zinc-500">{message.timestamp}</span>
          <p className="break-words text-sm text-zinc-200">
            <span
              className={
                message.author === "RobotX"
                  ? "font-semibold text-fuchsia-400"
                  : playerId && message.authorId === playerId
                    ? "font-semibold text-amber-300"
                    : "font-semibold text-emerald-400"
              }
            >
              {message.author}
            </span>
            <span className="text-zinc-500">: </span>
            {message.text}
          </p>
        </div>
      ))}
      <div ref={bottomRef} />
    </div>
  );
}
