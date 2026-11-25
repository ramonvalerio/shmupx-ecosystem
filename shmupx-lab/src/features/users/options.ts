import type { AdminUser, ChatReport, ChatReportReason, UserStatus } from "../../lib/backend";

/** Motivos de denúncia (os mesmos do Client e do Lite, `data/chatSafety.ts`). */
export const REPORT_REASON_LABEL: Record<ChatReportReason, string> = {
  abuse: "Ofensa ou assédio",
  spam: "Spam ou golpe",
  inappropriate: "Conteúdo impróprio",
  other: "Outro motivo",
};

export const REPORT_STATUS_LABEL: Record<ChatReport["status"], string> = {
  open: "Aberta",
  actioned: "Resolvida",
  dismissed: "Dispensada",
};

export const STATUS_LABEL: Record<UserStatus, string> = {
  live: "Ao vivo",
  playing: "Jogando",
  online: "Online",
  away: "Ausente",
  offline: "Offline",
};

/** Cor do ponto de cada situação. */
export const STATUS_DOT: Record<UserStatus, string> = {
  live: "bg-red-400 shadow-[0_0_8px_var(--color-red-400)]",
  playing: "bg-cyan-300 shadow-[0_0_8px_var(--color-cyan-300)]",
  online: "bg-emerald-400 shadow-[0_0_8px_var(--color-emerald-400)]",
  away: "bg-amber-400",
  offline: "bg-zinc-600",
};

/** Nome para mostrar: nome do perfil, senão @nick, senão e-mail, senão o uid. */
export function userName(user: Pick<AdminUser, "display_name" | "handle" | "email" | "uid">): string {
  return user.display_name || (user.handle ? `@${user.handle}` : "") || user.email || user.uid;
}

/** "agora", "há 5 min", "há 3 h", "há 2 dias" ou a data. */
export function timeAgo(iso: string | undefined, now = Date.now()): string {
  if (!iso) return "—";
  const seconds = Math.max(0, Math.round((now - new Date(iso).getTime()) / 1000));
  if (seconds < 60) return "agora";
  if (seconds < 3600) return `há ${Math.floor(seconds / 60)} min`;
  if (seconds < 86_400) return `há ${Math.floor(seconds / 3600)} h`;
  if (seconds < 30 * 86_400) return `há ${Math.floor(seconds / 86_400)} dia(s)`;
  return new Date(iso).toLocaleDateString("pt-BR");
}

export function isModerated(user: AdminUser): boolean {
  const m = user.moderation;
  return m.banned || m.muted || m.disabled || m.chat_hidden;
}

/** Busca por nome, @nick, e-mail ou uid, sem diferenciar maiúsculas nem acentos. */
export function matchesUser(user: AdminUser, search: string): boolean {
  const normalize = (text: string) => text.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
  const term = normalize(search.trim().replace(/^@/, ""));
  if (!term) return true;
  return [user.display_name, user.handle, user.email, user.uid].some((field) => field && normalize(field).includes(term));
}
