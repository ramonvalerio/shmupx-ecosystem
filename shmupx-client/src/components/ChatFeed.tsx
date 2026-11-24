import { Fragment, useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";
import { linkParts } from "../data/chatLinks";
import type { ChatMessage } from "../types/room";
import type { ChatStatus } from "../data/useRoomChat";
import { censor } from "../data/profanity";
import { avatarOf, usePublicProfiles } from "../data/publicProfile";
import { REPORT_REASONS, reportMessage, useBlockedAuthors, type ReportReason } from "../data/chatSafety";

interface ChatFeedProps {
  messages: ChatMessage[];
  status?: ChatStatus;
  error?: string | null;
  /** uid do jogador conectado: as mensagens dele aparecem destacadas. */
  playerId?: string;
  /** Sala do chat: com ela (e o jogador), cada mensagem de outro jogador ganha Denunciar e Bloquear. */
  roomId?: string;
  /** Texto quando não há mensagens (a visão "Só RobotX" tem o seu). */
  emptyText?: string;
}

/** Menu aberto numa mensagem: escolher a ação, o motivo da denúncia ou confirmar o bloqueio. */
type Panel = { id: string; stage: "menu" | "report" | "block" | "busy" } | { id: string; stage: "done"; text: string; failed?: boolean };

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

export default function ChatFeed({ messages, status = "ready", error, playerId, roomId, emptyText }: ChatFeedProps) {
  const { t, locale } = useI18n();
  const bottomRef = useRef<HTMLDivElement>(null);
  // Avatar de cada autor (perfil público), atualizado quando alguém troca.
  const profiles = usePublicProfiles(messages.map((m) => m.authorId ?? ""));
  const { blocked, block, unblock } = useBlockedAuthors(playerId);
  const [panel, setPanel] = useState<Panel | null>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [messages]);

  async function sendReport(message: ChatMessage, reason: ReportReason) {
    if (!playerId || !roomId) return;
    setPanel({ id: message.id, stage: "busy" });
    try {
      const outcome = await reportMessage(playerId, roomId, message, reason);
      setPanel({ id: message.id, stage: "done", text: outcome === "already" ? t("Você já denunciou esta mensagem.") : t("Denúncia enviada. A moderação do ShmupX vai analisar.") });
    } catch (failure) {
      console.warn("falha ao denunciar a mensagem:", failure);
      setPanel({ id: message.id, stage: "done", failed: true, text: t("Não foi possível enviar a denúncia. Tente de novo.") });
    }
  }

  async function confirmBlock(message: ChatMessage) {
    if (!message.authorId) return;
    setPanel({ id: message.id, stage: "busy" });
    try {
      await block(message.authorId);
      setPanel(null);
    } catch (failure) {
      console.warn("falha ao bloquear o jogador:", failure);
      setPanel({ id: message.id, stage: "done", failed: true, text: t("Não foi possível bloquear agora. Tente de novo.") });
    }
  }

  return (
    <div className="chat-feed" aria-live="polite" aria-busy={status === "loading"}>
      {status === "loading" && <p className="empty-state">{t("Carregando o chat da sala…")}</p>}
      {status === "error" && <p className="empty-state" role="alert">{t(error ?? "Não foi possível conectar ao chat da sala.")}</p>}
      {status === "ready" && messages.length === 0 && <p className="empty-state">{emptyText ?? t("Nenhuma mensagem ainda. Diga oi para a sala!")}</p>}
      {messages.map((message) => {
        // Só a resposta gerada neste PC é do RobotX: um jogador chamado "RobotX" não vira bot.
        const bot = Boolean(message.local) && message.author === "RobotX";
        const mine = Boolean(playerId && message.authorId === playerId);
        const authorId = message.authorId;
        // Mensagem de quem o jogador bloqueou: só um aviso, com o caminho para desbloquear.
        if (authorId && !mine && blocked.has(authorId)) {
          return (
            <article key={message.id} className="chat-message is-blocked">
              <p className="chat-blocked">
                {t("Mensagem de um jogador que você bloqueou.")}{" "}
                <button type="button" className="chat-link-button" onClick={() => void unblock(authorId).catch((failure) => console.warn("falha ao desbloquear:", failure))}>{t("Desbloquear")}</button>
              </p>
            </article>
          );
        }
        const actionable = Boolean(roomId && playerId && authorId && !mine && !bot && !message.local && !message.pending);
        const open = panel?.id === message.id ? panel : null;
        return (
          <article key={message.id} className={`chat-message ${bot ? "is-bot" : ""} ${mine ? "is-mine" : ""}`} style={message.pending ? { opacity: 0.6 } : undefined}>
            <span className="chat-avatar" aria-hidden="true">{bot ? "RX" : <img className="avatar-img" src={avatarOf(profiles, message.authorId)} alt="" />}</span>
            <div className="chat-body">
              <header>
                <strong>{message.author}{mine ? ` ${t("(você)")}` : ""}</strong>
                {bot && <small className="chat-tag">{t("BOT")}</small>}
                {bot && <small className="chat-tag is-private">🔒 {t("Reservado para você")}</small>}
                <time dateTime={message.timestamp}>{horario(message.timestamp, locale)}</time>
                {actionable && (
                  <button type="button" className="chat-more" aria-expanded={Boolean(open)} aria-label={t("Denunciar ou bloquear {author}", { author: message.author })} title={t("Denunciar ou bloquear")}
                    onClick={() => setPanel(open ? null : { id: message.id, stage: "menu" })}>⋯</button>
                )}
              </header>
              {message.inReplyTo && <blockquote className="chat-reply-to">{message.inReplyTo}</blockquote>}
              {/* Mascara de novo na exibição: mensagens antigas ou de quem burlou o envio. */}
              <p><ChatText text={bot ? message.text : censor(message.text).text} /></p>
              {open && (
                <div className="chat-actions" role="group" aria-label={t("Denunciar ou bloquear")}>
                  {open.stage === "menu" && <>
                    <button type="button" onClick={() => setPanel({ id: message.id, stage: "report" })}>{t("Denunciar mensagem")}</button>
                    <button type="button" onClick={() => setPanel({ id: message.id, stage: "block" })}>{t("Bloquear {author}", { author: message.author })}</button>
                    <button type="button" className="is-quiet" onClick={() => setPanel(null)}>{t("Cancelar")}</button>
                  </>}
                  {open.stage === "report" && <>
                    <span className="chat-actions-label">{t("Denunciar por:")}</span>
                    {REPORT_REASONS.map((reason) => <button key={reason.id} type="button" onClick={() => void sendReport(message, reason.id)}>{t(reason.label)}</button>)}
                    <button type="button" className="is-quiet" onClick={() => setPanel(null)}>{t("Cancelar")}</button>
                  </>}
                  {open.stage === "block" && <>
                    <span className="chat-actions-label">{t("Bloquear {author}? Você deixa de ver as mensagens dessa pessoa no chat, no PC e no celular.", { author: message.author })}</span>
                    <button type="button" onClick={() => void confirmBlock(message)}>{t("Bloquear")}</button>
                    <button type="button" className="is-quiet" onClick={() => setPanel(null)}>{t("Cancelar")}</button>
                  </>}
                  {open.stage === "busy" && <span className="chat-actions-label">{t("Enviando…")}</span>}
                  {open.stage === "done" && <>
                    <span className={`chat-actions-label ${open.failed ? "is-error" : ""}`} role="status">{open.text}</span>
                    <button type="button" className="is-quiet" onClick={() => setPanel(null)}>{t("Fechar")}</button>
                  </>}
                </div>
              )}
            </div>
          </article>
        );
      })}
      <div ref={bottomRef} />
    </div>
  );
}
