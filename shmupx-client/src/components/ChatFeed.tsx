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
  /** Texto quando não há mensagens (a visão "Só RobotX" tem o seu). */
  emptyText?: string;
}

/** Hora curta da mensagem (o timestamp vem em ISO); texto que não é data aparece como veio. */
function hora(timestamp: string, locale: string): string {
  const data = new Date(timestamp);
  return Number.isNaN(data.getTime()) ? timestamp : data.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });
}

export default function ChatFeed({ messages, status = "ready", error, playerId, emptyText }: ChatFeedProps) {
  const { t, locale } = useI18n();
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [messages]);

  return (
    <div className="chat-feed" aria-live="polite" aria-busy={status === "loading"}>
      {status === "loading" && <p className="empty-state">{t("Carregando o chat da sala…")}</p>}
      {status === "error" && <p className="empty-state" role="alert">{t(error ?? "Não foi possível conectar ao chat da sala.")}</p>}
      {status === "ready" && messages.length === 0 && <p className="empty-state">{emptyText ?? t("Nenhuma mensagem ainda. Diga oi para a sala!")}</p>}
      {messages.map((message) => {
        // Só a resposta gerada neste PC é do RobotX: um jogador chamado "RobotX" não vira bot.
        const bot = Boolean(message.local) && message.author === "RobotX";
        const mine = Boolean(playerId && message.authorId === playerId);
        return (
          <article key={message.id} className={`chat-message ${bot ? "is-bot" : ""} ${mine ? "is-mine" : ""}`} style={message.pending ? { opacity: 0.6 } : undefined}>
            <span className="chat-avatar" aria-hidden="true">{bot ? "RX" : <img className="avatar-img" src="/img/avatar-default.png" alt="" />}</span>
            <div className="chat-body">
              <header>
                <strong>{message.author}{mine ? ` ${t("(você)")}` : ""}</strong>
                {bot && <small className="chat-tag">{t("BOT")}</small>}
                {bot && <small className="chat-tag is-private">🔒 {t("Reservado para você")}</small>}
                <time dateTime={message.timestamp}>{hora(message.timestamp, locale)}</time>
              </header>
              {message.inReplyTo && <blockquote className="chat-reply-to">{message.inReplyTo}</blockquote>}
              <p>{message.text}</p>
            </div>
          </article>
        );
      })}
      <div ref={bottomRef} />
    </div>
  );
}
