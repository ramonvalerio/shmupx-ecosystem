import type { CoinGrant, CoinGrantReason } from "../../lib/backend";

/** Motivos de envio de fichas (mesma lista do backend, `services/distribuicoes.rs::MOTIVOS`). */
export const REASONS: { value: CoinGrantReason; label: string }[] = [
  { value: "event_prize", label: "Prêmio de evento ou torneio" },
  { value: "season_reward", label: "Recompensa de temporada" },
  { value: "compensation", label: "Compensação por instabilidade" },
  { value: "support_refund", label: "Reembolso (suporte)" },
  { value: "promotion", label: "Promoção" },
  { value: "welcome_bonus", label: "Bônus de boas-vindas" },
  { value: "bug_report", label: "Recompensa por relato de bug" },
  { value: "community", label: "Participação na comunidade" },
  { value: "content_creator", label: "Criador de conteúdo" },
  { value: "other", label: "Outro (descreva na observação)" },
];

export const REASON_LABEL = Object.fromEntries(REASONS.map((r) => [r.value, r.label.replace(/ \(descreva.*\)$/, "")])) as Record<CoinGrantReason, string>;

export const GRANT_STATUS_LABEL: Record<CoinGrant["status"], string> = {
  scheduled: "Agendada",
  running: "Creditando…",
  done: "Creditada",
  cancelled: "Cancelada",
  failed: "Falhou",
};

export const GRANT_STATUS_TONE: Record<CoinGrant["status"], "info" | "warning" | "success" | "neutral" | "danger"> = {
  scheduled: "info",
  running: "warning",
  done: "success",
  cancelled: "neutral",
  failed: "danger",
};

/** Valor de `<input type="datetime-local">` no fuso do computador. */
export function toLocalInput(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

export function formatDateTime(iso: string | Date | undefined): string {
  if (!iso) return "—";
  return new Date(iso).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
}

export const plural = (n: number, one: string, many: string) => `${n.toLocaleString("pt-BR")} ${n === 1 ? one : many}`;
