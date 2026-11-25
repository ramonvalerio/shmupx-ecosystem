import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Badge from "../../components/ui/Badge";
import Button from "../../components/ui/Button";
import { Field, Select, TextInput } from "../../components/ui/Field";
import { IconAlert, IconCalendar, IconCheck, IconClose, IconCoins, IconRefresh, IconSearch } from "../../components/ui/Icons";
import { cancelCoinGrant, createCoinGrant, listCoinGrants, retryCoinGrant, type AdminUser, type CoinGrant, type CoinGrantReason } from "../../lib/backend";
import { matchesUser, userName } from "../users/options";
import { formatDateTime, GRANT_STATUS_LABEL, GRANT_STATUS_TONE, plural, REASON_LABEL, REASONS, toLocalInput } from "./options";

/** Agendadas viram creditadas sozinhas: a lista acompanha enquanto a tela está aberta. */
const ATUALIZAR_A_CADA_MS = 15_000;
const QUICK_AMOUNTS = [1, 5, 10, 25, 50];

interface CoinsViewProps {
  users: AdminUser[];
  usersLoading: boolean;
  /** Jogador escolhido no Users Manager ("Enviar fichas"). */
  preselectedUid: string | null;
  onPreselectedUsed: () => void;
}

/** Coins Manager: envia fichas na hora ou agenda, para todos os jogadores ou um só, com motivo. */
export default function CoinsView({ users, usersLoading, preselectedUid, onPreselectedUsed }: CoinsViewProps) {
  const [grants, setGrants] = useState<CoinGrant[]>([]);
  const [loading, setLoading] = useState(false);
  const [listError, setListError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  const reload = useCallback(async (quiet = false) => {
    if (!quiet) setLoading(true);
    try {
      setGrants(await listCoinGrants());
      setListError(null);
    } catch (e) {
      setListError(e instanceof Error ? e.message : String(e));
    } finally {
      if (!quiet) setLoading(false);
    }
  }, []);

  useEffect(() => {
    void reload();
    const timer = window.setInterval(() => void reload(true), ATUALIZAR_A_CADA_MS);
    return () => window.clearInterval(timer);
  }, [reload]);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 5000);
    return () => window.clearTimeout(timer);
  }, [notice]);

  const upsert = (grant: CoinGrant) =>
    setGrants((current) => [grant, ...current.filter((g) => g.id !== grant.id)].sort((a, b) => b.scheduled_at.localeCompare(a.scheduled_at)));

  async function act(grant: CoinGrant, action: "cancel" | "retry") {
    setBusyId(grant.id);
    try {
      const saved = action === "cancel" ? await cancelCoinGrant(grant.id) : await retryCoinGrant(grant.id);
      upsert(saved);
      setNotice(action === "cancel" ? "Distribuição cancelada." : saved.status === "done" ? `Fichas creditadas para ${plural(saved.recipients, "jogador", "jogadores")}.` : "Distribuição rodou de novo.");
    } catch (e) {
      setListError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusyId(null);
    }
  }

  const scheduled = grants.filter((g) => g.status === "scheduled");
  const userLabel = (grant: CoinGrant) => {
    if (grant.target === "all") return "Todos os jogadores";
    const user = users.find((u) => u.uid === grant.uid);
    return user ? userName(user) : grant.user_label || grant.uid || "—";
  };

  return (
    <div className="relative flex min-w-0 flex-1 overflow-hidden">
      <section className="flex min-w-0 flex-1 flex-col">
        <header className="flex flex-wrap items-end justify-between gap-4 px-8 pt-7 pb-5">
          <div>
            <h1 className="text-xl font-semibold text-zinc-100">Coins Manager</h1>
            <p className="mt-1 max-w-2xl text-sm text-zinc-500">
              Envie fichas para todos os jogadores ou para um só, na hora ou agendado. Cada envio tem um motivo e fica no extrato da carteira do jogador.
            </p>
          </div>
          <div className="flex items-center gap-3">
            {scheduled.length > 0 && <span className="text-xs text-zinc-500">{plural(scheduled.length, "agendada", "agendadas")}</span>}
            <Button variant="ghost" onClick={() => void reload()} disabled={loading} title="Recarregar" aria-label="Recarregar">
              <IconRefresh className={loading ? "animate-spin" : ""} />
            </Button>
          </div>
        </header>

        <div className="flex min-h-0 flex-1 gap-6 px-8 pb-8">
          <GrantForm
            users={users}
            usersLoading={usersLoading}
            preselectedUid={preselectedUid}
            onPreselectedUsed={onPreselectedUsed}
            onCreated={(grant) => {
              upsert(grant);
              setNotice(
                grant.status === "done"
                  ? `${plural(grant.amount, "ficha creditada", "fichas creditadas")} para ${grant.target === "all" ? plural(grant.recipients, "jogador", "jogadores") : userLabel(grant)}.`
                  : `Distribuição agendada para ${formatDateTime(grant.scheduled_at)}.`,
              );
            }}
          />

          <div className="flex min-w-0 flex-1 flex-col">
            {listError && (
              <p role="alert" className="mb-4 flex gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
                <IconAlert className="mt-0.5 flex-shrink-0" />
                {listError}
              </p>
            )}
            <div className="min-h-0 flex-1 overflow-auto rounded-lg border border-white/5 bg-zinc-950/40">
              {grants.length === 0 ? (
                <div className="flex h-full min-h-56 flex-col items-center justify-center gap-2 px-6 py-14 text-center">
                  <IconCoins className="h-6 w-6 text-zinc-600" />
                  <p className="text-sm font-medium text-zinc-200">{loading ? "Carregando…" : "Nenhuma distribuição ainda"}</p>
                  {!loading && <p className="max-w-md text-xs text-zinc-500">Use o formulário ao lado para enviar ou agendar fichas.</p>}
                </div>
              ) : (
                <table className="w-full text-left text-sm">
                  <thead className="sticky top-0 bg-zinc-950 text-[11px] tracking-wider text-zinc-500 uppercase">
                    <tr>
                      <th className="px-4 py-3 font-semibold">Quando</th>
                      <th className="px-4 py-3 font-semibold">Para</th>
                      <th className="px-4 py-3 text-right font-semibold">Fichas</th>
                      <th className="px-4 py-3 font-semibold">Motivo</th>
                      <th className="px-4 py-3 font-semibold">Situação</th>
                      <th className="px-4 py-3" />
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-white/5">
                    {grants.map((grant) => (
                      <tr key={grant.id} className="align-top">
                        <td className="px-4 py-3 text-xs whitespace-nowrap text-zinc-300 tabular-nums">
                          {formatDateTime(grant.scheduled_at)}
                          <span className="mt-0.5 block text-[11px] text-zinc-600" title={`Criada em ${formatDateTime(grant.created_at)}`}>por {grant.created_by}</span>
                        </td>
                        <td className="px-4 py-3 text-zinc-200">
                          {userLabel(grant)}
                          {grant.status === "done" && grant.target === "all" && <span className="mt-0.5 block text-[11px] text-zinc-500">{plural(grant.recipients, "jogador creditado", "jogadores creditados")}</span>}
                        </td>
                        <td className="px-4 py-3 text-right font-mono text-cyan-200 tabular-nums">+{grant.amount.toLocaleString("pt-BR")}</td>
                        <td className="max-w-64 px-4 py-3 text-xs text-zinc-300">
                          {REASON_LABEL[grant.reason] ?? grant.reason}
                          {grant.note && <span className="mt-0.5 block text-zinc-500">{grant.note}</span>}
                        </td>
                        <td className="px-4 py-3">
                          <Badge tone={GRANT_STATUS_TONE[grant.status]}>{GRANT_STATUS_LABEL[grant.status]}</Badge>
                          {grant.status === "done" && grant.executed_at && <span className="mt-1 block text-[11px] text-zinc-600">{formatDateTime(grant.executed_at)}</span>}
                          {grant.error && <span className="mt-1 block max-w-56 text-[11px] text-red-300">{grant.error}</span>}
                        </td>
                        <td className="px-4 py-3 text-right">
                          {grant.status === "scheduled" && (
                            <Button size="sm" variant="ghost" loading={busyId === grant.id} onClick={() => void act(grant, "cancel")}>Cancelar</Button>
                          )}
                          {grant.status === "failed" && (
                            <Button size="sm" variant="secondary" loading={busyId === grant.id} onClick={() => void act(grant, "retry")}>Repetir</Button>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
            <p className="mt-2 text-[11px] text-zinc-600">
              Agendadas são creditadas pelo backend quando chega a hora, no primeiro acesso de um jogador ou do Lab (o backend dorme sem ninguém conectado).
            </p>
          </div>
        </div>
      </section>

      {notice && (
        <div role="status" className="pointer-events-none fixed right-6 bottom-6 z-30 flex items-center gap-2 rounded-lg border border-emerald-400/20 bg-zinc-900 px-4 py-3 text-sm text-zinc-100 shadow-xl shadow-black/40">
          <IconCheck className="text-emerald-300" />
          {notice}
        </div>
      )}
    </div>
  );
}

type When = "now" | "schedule";

function GrantForm({ users, usersLoading, preselectedUid, onPreselectedUsed, onCreated }: {
  users: AdminUser[];
  usersLoading: boolean;
  preselectedUid: string | null;
  onPreselectedUsed: () => void;
  onCreated: (grant: CoinGrant) => void;
}) {
  const [target, setTarget] = useState<"all" | "user">("user");
  const [uid, setUid] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [amount, setAmount] = useState("10");
  const [reason, setReason] = useState<CoinGrantReason>("event_prize");
  const [note, setNote] = useState("");
  const [when, setWhen] = useState<When>("now");
  const [scheduledAt, setScheduledAt] = useState(() => toLocalInput(new Date(Date.now() + 3600_000)));
  const [confirming, setConfirming] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  // "Enviar fichas" no Users Manager abre aqui com o jogador escolhido.
  useEffect(() => {
    if (!preselectedUid) return;
    setTarget("user");
    setUid(preselectedUid);
    setSearch("");
    onPreselectedUsed();
  }, [preselectedUid, onPreselectedUsed]);

  // Qualquer mudança pede a confirmação de novo.
  useEffect(() => setConfirming(false), [target, uid, amount, reason, note, when, scheduledAt]);

  const selected = uid ? users.find((u) => u.uid === uid) ?? null : null;
  const matches = useMemo(
    () => (search.trim() ? users.filter((u) => matchesUser(u, search) && !u.moderation.banned).slice(0, 8) : []),
    [users, search],
  );
  const recipients = target === "all" ? users.filter((u) => !u.moderation.banned).length : selected ? 1 : 0;
  const quantity = Number(amount);
  const scheduledDate = when === "schedule" ? new Date(scheduledAt) : null;

  const problem =
    target === "user" && !uid ? "Escolha o jogador."
    : !Number.isInteger(quantity) || quantity < 1 || quantity > 10_000 ? "Quantidade: de 1 a 10.000 fichas."
    : reason === "other" && !note.trim() ? "No motivo \"Outro\", descreva na observação."
    : scheduledDate && (Number.isNaN(scheduledDate.getTime()) || scheduledDate.getTime() < Date.now() + 60_000) ? "Agende para pelo menos 1 minuto à frente."
    : null;

  const summary = `${plural(Number.isFinite(quantity) ? quantity : 0, "ficha", "fichas")} para ${
    target === "all" ? `todos os jogadores (${plural(recipients, "conta", "contas")})` : selected ? userName(selected) : "o jogador escolhido"
  } ${scheduledDate ? `em ${formatDateTime(scheduledDate)}` : "agora"}`;

  async function submit() {
    if (problem) return;
    // Para todos, ou muitas fichas: pede um segundo clique.
    if (!confirming && (target === "all" || quantity >= 100)) {
      setConfirming(true);
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const grant = await createCoinGrant({
        target,
        uid: target === "user" ? uid ?? undefined : undefined,
        amount: quantity,
        reason,
        note: note.trim() || undefined,
        scheduled_at: scheduledDate ? scheduledDate.toISOString() : undefined,
      });
      onCreated(grant);
      setConfirming(false);
      setNote("");
      if (target === "user") setUid(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  const segment = (active: boolean) =>
    `flex-1 rounded-md px-3 py-1.5 text-xs font-medium transition-colors focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none ${
      active ? "bg-cyan-400 text-zinc-950" : "text-zinc-400 hover:bg-white/5 hover:text-zinc-100"
    }`;

  return (
    <form
      onSubmit={(event) => { event.preventDefault(); void submit(); }}
      aria-label="Nova distribuição de fichas"
      className="flex w-[380px] flex-shrink-0 flex-col gap-4 self-start rounded-lg border border-white/5 bg-zinc-950/60 p-5"
    >
      <p className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">Nova distribuição</p>

      <div>
        <span className="mb-1.5 block text-xs font-medium text-zinc-400">Para quem</span>
        <div role="radiogroup" aria-label="Para quem" className="flex gap-1 rounded-lg border border-white/10 bg-zinc-950 p-1">
          <button type="button" role="radio" aria-checked={target === "user"} className={segment(target === "user")} onClick={() => setTarget("user")}>Um jogador</button>
          <button type="button" role="radio" aria-checked={target === "all"} className={segment(target === "all")} onClick={() => setTarget("all")}>Todos os jogadores</button>
        </div>
      </div>

      {target === "user" && (
        <div>
          <span className="mb-1.5 block text-xs font-medium text-zinc-400">Jogador</span>
          {selected ? (
            <div className="flex items-center gap-3 rounded-md border border-cyan-400/30 bg-cyan-400/5 px-3 py-2">
              <Initial user={selected} />
              <div className="min-w-0 flex-1 leading-tight">
                <p className="truncate text-sm text-zinc-100">{userName(selected)}</p>
                <p className="truncate text-[11px] text-zinc-500">{[selected.handle && `@${selected.handle}`, selected.email].filter(Boolean).join(" · ")}</p>
              </div>
              <span className="text-xs text-zinc-500 tabular-nums" title="Saldo atual">{selected.balance ?? 50} fichas</span>
              <button type="button" aria-label="Trocar jogador" onClick={() => { setUid(null); window.setTimeout(() => searchRef.current?.focus(), 0); }} className="rounded p-1 text-zinc-500 hover:bg-white/5 hover:text-zinc-200">
                <IconClose />
              </button>
            </div>
          ) : (
            <div className="relative">
              <IconSearch className="pointer-events-none absolute top-2.5 left-3 text-zinc-600" />
              <TextInput ref={searchRef} value={search} onChange={(e) => setSearch(e.target.value)} placeholder={usersLoading && users.length === 0 ? "Carregando jogadores…" : "Nome, @nick, e-mail ou uid"} className="pl-9" aria-label="Buscar jogador" />
              {matches.length > 0 && (
                <ul role="listbox" aria-label="Jogadores encontrados" className="absolute inset-x-0 top-full z-10 mt-1 max-h-72 overflow-auto rounded-md border border-white/10 bg-zinc-900 py-1 shadow-xl shadow-black/50">
                  {matches.map((user) => (
                    <li key={user.uid}>
                      <button type="button" role="option" aria-selected={false} onClick={() => { setUid(user.uid); setSearch(""); }} className="flex w-full items-center gap-3 px-3 py-2 text-left hover:bg-white/5">
                        <Initial user={user} />
                        <span className="min-w-0 flex-1 leading-tight">
                          <span className="block truncate text-sm text-zinc-100">{userName(user)}</span>
                          <span className="block truncate text-[11px] text-zinc-500">{[user.handle && `@${user.handle}`, user.email].filter(Boolean).join(" · ")}</span>
                        </span>
                      </button>
                    </li>
                  ))}
                </ul>
              )}
              {search.trim() && matches.length === 0 && !usersLoading && <p className="mt-1.5 text-xs text-zinc-500">Nenhum jogador encontrado.</p>}
            </div>
          )}
        </div>
      )}

      <Field label="Quantidade de fichas (por jogador)">
        <TextInput type="number" min={1} max={10_000} step={1} value={amount} onChange={(e) => setAmount(e.target.value)} />
      </Field>
      <div className="-mt-2 flex gap-1.5">
        {QUICK_AMOUNTS.map((n) => (
          <button key={n} type="button" onClick={() => setAmount(String(n))} aria-pressed={quantity === n} className={`rounded-md border px-2.5 py-1 text-xs tabular-nums transition-colors ${quantity === n ? "border-cyan-400/60 bg-cyan-400/10 text-cyan-200" : "border-white/10 text-zinc-400 hover:border-white/20 hover:text-zinc-200"}`}>
            {n}
          </button>
        ))}
      </div>

      <Field label="Motivo">
        <Select value={reason} onChange={(e) => setReason(e.target.value as CoinGrantReason)}>
          {REASONS.map((r) => <option key={r.value} value={r.value}>{r.label}</option>)}
        </Select>
      </Field>

      <Field label={reason === "other" ? "Observação (obrigatória)" : "Observação (opcional)"} hint="Fica registrada na distribuição, só para o Lab.">
        <TextInput value={note} maxLength={200} onChange={(e) => setNote(e.target.value)} placeholder="Ex.: 1º lugar no torneio de novembro" />
      </Field>

      <div>
        <span className="mb-1.5 block text-xs font-medium text-zinc-400">Quando</span>
        <div role="radiogroup" aria-label="Quando" className="flex gap-1 rounded-lg border border-white/10 bg-zinc-950 p-1">
          <button type="button" role="radio" aria-checked={when === "now"} className={segment(when === "now")} onClick={() => setWhen("now")}>Agora</button>
          <button type="button" role="radio" aria-checked={when === "schedule"} className={segment(when === "schedule")} onClick={() => setWhen("schedule")}>
            <span className="inline-flex items-center gap-1.5"><IconCalendar className="h-3.5 w-3.5" />Agendar</span>
          </button>
        </div>
        {when === "schedule" && (
          <TextInput type="datetime-local" className="mt-2 [color-scheme:dark]" value={scheduledAt} min={toLocalInput(new Date())} onChange={(e) => setScheduledAt(e.target.value)} aria-label="Data e hora do envio" />
        )}
      </div>

      <div className="rounded-md border border-white/5 bg-black/30 px-3 py-2.5 text-xs leading-relaxed text-zinc-400">
        {problem ? <span className="text-amber-200">{problem}</span> : <>Vai creditar <span className="text-zinc-100">{summary}</span>.{confirming && <span className="mt-1 block text-amber-200">Confira e clique em confirmar.</span>}</>}
      </div>

      {error && <p role="alert" className="text-xs text-red-300">{error}</p>}

      <Button type="submit" variant={confirming ? "success" : "primary"} loading={saving} disabled={Boolean(problem)}>
        {confirming ? (when === "now" ? "Confirmar envio" : "Confirmar agendamento") : when === "now" ? "Enviar fichas agora" : "Agendar distribuição"}
      </Button>
      {confirming && (
        <button type="button" onClick={() => setConfirming(false)} className="-mt-2 text-xs text-zinc-500 hover:text-zinc-300">Voltar e revisar</button>
      )}
    </form>
  );
}

function Initial({ user }: { user: AdminUser }) {
  return (
    <span aria-hidden className="grid h-7 w-7 flex-shrink-0 place-items-center rounded-full bg-zinc-800 text-xs font-semibold text-zinc-300 uppercase">
      {userName(user).replace(/^@/, "").charAt(0)}
    </span>
  );
}
