import type { ClientRelease, ReleaseLead } from "../../lib/backend";

export const LEADS: { value: ReleaseLead; label: string; hint: string }[] = [
  { value: "urgent", label: "Urgente", hint: "Libera agora: o aviso e a versão chegam juntos." },
  { value: "24h", label: "24 horas", hint: "Avisa agora e libera em 24 horas." },
  { value: "48h", label: "48 horas", hint: "Avisa agora e libera em 48 horas." },
  { value: "7d", label: "7 dias", hint: "Para mudanças que pedem preparo (ex.: regra de ranking)." },
];

const LEAD_MS: Record<ReleaseLead, number> = { urgent: 0, "24h": 24 * 3600_000, "48h": 48 * 3600_000, "7d": 7 * 24 * 3600_000 };

export const STATUS_LABEL: Record<ClientRelease["status"], string> = {
  draft: "Rascunho",
  scheduled: "Agendada",
  released: "Liberada",
  cancelled: "Cancelada",
};

export const STATUS_TONE: Record<ClientRelease["status"], "warning" | "info" | "success" | "danger"> = {
  draft: "warning",
  scheduled: "info",
  released: "success",
  cancelled: "danger",
};

export function formatDateTime(iso: string | undefined | Date): string {
  if (!iso) return "—";
  return new Date(iso).toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
}

/** Mesmo texto que o backend publica no aviso (services/notificacoes.rs::textos_da_versao). */
export function noticePreview(version: string, title: string, mandatory: boolean, lead: ReleaseLead, now = new Date()) {
  const releaseAt = new Date(now.getTime() + LEAD_MS[lead]);
  const when = lead === "urgent" ? "Disponível agora." : `Chega em ${formatDateTime(releaseAt)}.`;
  return {
    title: `ShmupX Client ${version}: ${title || "(sem título)"}`,
    summary: `${when}${mandatory ? " Atualização obrigatória para continuar jogando." : ""}`,
    releaseAt,
  };
}

/** Texto com um item por linha ↔ lista. */
export const toLines = (items: string[]) => items.join("\n");
export const fromLines = (text: string) => text.split("\n").map((line) => line.trim()).filter(Boolean);

/** Resumo do envio dos e-mails para a tela. */
export function emailLabel(release: ClientRelease): string | null {
  const status = release.email_status;
  if (!status) return null;
  if (status === "not_configured") return "E-mail não configurado no backend (RESEND_API_KEY/EMAIL_FROM): só o aviso no Client.";
  if (status === "skipped") return "Sem e-mail (desligado ao agendar): só o aviso no Client.";
  if (status === "sending") return "Enviando e-mails…";
  if (status === "sent") return `${release.emails_sent ?? 0} e-mail(s) enviado(s).`;
  return `Falha no envio de e-mails: ${status.replace(/^failed: /, "")}`;
}
