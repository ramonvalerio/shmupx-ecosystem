import { Fragment, useEffect, useRef } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import { linkParts } from "../data/chatLinks";
import type { ChatMessage } from "../types/room";
import type { ChatStatus } from "../data/useRoomChat";
import { censor } from "../data/profanity";
import { avatarOf, usePublicProfiles } from "../data/publicProfile";

interface ChatFeedProps {
  messages: ChatMessage[];
  status?: ChatStatus;
  error?: string | null;
  /** uid do jogador conectado: as mensagens dele aparecem destacadas. */
  playerId?: string;
  /** Texto quando não há mensagens (a visão "Só RobotX" tem o seu). */
  emptyText?: string;
}

/**
 * Horário da mensagem no formato do idioma: só a hora quando é de hoje; de outro dia, hora e data
 * (pt-BR: "15:55 20/11/2026"; en: "03:55 PM 11/20/2026"). Texto que não é data aparece como veio.
 */
function horario(timestamp: string, locale: string, agora = new Date()): string {
  const data = new Date(timestamp);
  if (Number.isNaN(data.getTime())) return timestamp;
  const hora = data.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });
  if (data.toDateString() === agora.toDateString()) return hora;
  return `${hora} ${data.toLocaleDateString(locale, { day: "2-digit", month: "2-digit", year: "numeric" })}`;
}

/** Abre o link no navegador do sistema (nunca dentro do app). */
function openLink(url: string) {
  if (isTauri()) void invoke("chat_abrir_link", { url }).catch((error) => console.warn("falha ao abrir o link:", error));
  else window.open(url, "_blank", "noopener");
}

/** Texto da mensagem com os links clicáveis. */
function ChatText({ text }: { text: string }) {
  return <>{linkParts(text).map((part, index) => "url" in part
    ? <a key={index} className="chat-link" href={part.url} title={part.url} onClick={(event) => { event.preventDefault(); openLink(part.url); }}>{part.text}</a>
    : <Fragment key={index}>{part.text}</Fragment>)}</>;
}

export default function ChatFeed({ messages, status = "ready", error, playerId, emptyText }: ChatFeedProps) {
  const { t, locale } = useI18n();
  const bottomRef = useRef<HTMLDivElement>(null);
  // Avatar de cada autor (perfil público), atualizado quando alguém troca.
  const profiles = usePublicProfiles(messages.map((m) => m.authorId ?? ""));

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
            <span className="chat-avatar" aria-hidden="true">{bot ? "RX" : <img className="avatar-img" src={avatarOf(profiles, message.authorId)} alt="" />}</span>
            <div className="chat-body">
              <header>
                <strong>{message.author}{mine ? ` ${t("(você)")}` : ""}</strong>
                {bot && <small className="chat-tag">{t("BOT")}</small>}
                {bot && <small className="chat-tag is-private">🔒 {t("Reservado para você")}</small>}
                <time dateTime={message.timestamp}>{horario(message.timestamp, locale)}</time>
              </header>
              {message.inReplyTo && <blockquote className="chat-reply-to">{message.inReplyTo}</blockquote>}
              {/* Mascara de novo na exibição: mensagens antigas ou de quem burlou o envio. */}
              <p><ChatText text={bot ? message.text : censor(message.text).text} /></p>
            </div>
          </article>
        );
      })}
      <div ref={bottomRef} />
    </div>
  );
}
