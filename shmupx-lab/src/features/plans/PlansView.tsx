import { useCallback, useEffect, useState, type FormEvent } from "react";
import Badge from "../../components/ui/Badge";
import Button from "../../components/ui/Button";
import Switch from "../../components/ui/Switch";
import { Field, Select, TextInput } from "../../components/ui/Field";
import { IconAlert, IconRefresh } from "../../components/ui/Icons";
import { deletePlan, listPlans, savePlan, type Plan, type PlanPeriod } from "../../lib/backend";
import { formatDateTime } from "../coins/options";

const PERIODS: { value: PlanPeriod; label: string; short: string }[] = [
  { value: "monthly", label: "Mensal", short: "/mês" },
  { value: "quarterly", label: "Trimestral", short: "/trimestre" },
  { value: "semiannual", label: "Semestral", short: "/semestre" },
  { value: "yearly", label: "Anual", short: "/ano" },
];
const periodShort = (p: PlanPeriod) => PERIODS.find((x) => x.value === p)?.short ?? "";
const brl = (cents: number) => (cents / 100).toLocaleString("pt-BR", { style: "currency", currency: "BRL" });

interface Draft { id: string; name: string; description: string; price: string; period: PlanPeriod; coins: string; benefits: string; active: boolean; order: string }
const EMPTY: Draft = { id: "", name: "", description: "", price: "", period: "monthly", coins: "0", benefits: "", active: false, order: "0" };
const toDraft = (p: Plan): Draft => ({
  id: p.id, name: p.name, description: p.description, price: (p.price_cents / 100).toFixed(2).replace(".", ","), period: p.period,
  coins: String(p.coins_per_period), benefits: p.benefits.join("\n"), active: p.active, order: String(p.order),
});
/** "Plano Pro" -> "plano-pro" (o ID do documento). */
const slug = (text: string) => text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

/**
 * Plans: os planos de assinatura (`plans/{id}`): nome, preço por período, fichas incluídas e
 * benefícios. Por enquanto é o cadastro; a cobrança e quem assina chegam com o pagamento.
 */
export default function PlansView() {
  const [plans, setPlans] = useState<Plan[]>([]);
  const [loading, setLoading] = useState(false);
  const [listError, setListError] = useState<string | null>(null);
  const [draft, setDraft] = useState<Draft>(EMPTY);
  const [editing, setEditing] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const reload = useCallback(async () => {
    setLoading(true);
    setListError(null);
    try { setPlans(await listPlans()); } catch (e) { setListError(e instanceof Error ? e.message : String(e)); } finally { setLoading(false); }
  }, []);
  useEffect(() => { void reload(); }, [reload]);

  function edit(plan: Plan | null) {
    setEditing(plan?.id ?? null);
    setDraft(plan ? toDraft(plan) : EMPTY);
    setError(null);
    setNotice(null);
    setConfirmDelete(false);
  }

  const id = editing ?? (draft.id || slug(draft.name));
  const priceCents = Math.round(Number(draft.price.replace(/\./g, "").replace(",", ".")) * 100);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!id) { setError("Dê um nome ao plano."); return; }
    if (!Number.isFinite(priceCents) || priceCents < 0) { setError("Preço inválido (ex.: 19,90)."); return; }
    setSaving(true);
    setError(null);
    try {
      const saved = await savePlan(id, {
        name: draft.name, description: draft.description, price_cents: priceCents, period: draft.period,
        coins_per_period: Number(draft.coins) || 0, benefits: draft.benefits.split("\n"), active: draft.active, order: Number(draft.order) || 0,
      });
      setPlans((current) => [...current.filter((p) => p.id !== saved.id), saved].sort((a, b) => a.order - b.order || a.name.localeCompare(b.name)));
      edit(saved);
      setNotice(`Plano ${saved.name} salvo.`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  async function remove() {
    if (!editing) return;
    if (!confirmDelete) { setConfirmDelete(true); return; }
    setSaving(true);
    try {
      await deletePlan(editing);
      setPlans((current) => current.filter((p) => p.id !== editing));
      edit(null);
      setNotice("Plano excluído.");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="flex min-w-0 flex-1 flex-col overflow-hidden">
      <header className="flex flex-wrap items-end justify-between gap-4 px-8 pt-7 pb-5">
        <div>
          <h1 className="text-xl font-semibold text-zinc-100">Plans</h1>
          <p className="mt-1 max-w-2xl text-sm text-zinc-500">Planos de assinatura: o que cada um inclui e quanto custa. Desativado, o plano fica guardado mas não é oferecido.</p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="secondary" size="sm" onClick={() => edit(null)}>Novo plano</Button>
          <Button variant="ghost" onClick={() => void reload()} disabled={loading} title="Recarregar" aria-label="Recarregar"><IconRefresh className={loading ? "animate-spin" : ""} /></Button>
        </div>
      </header>

      <div className="flex min-h-0 flex-1 gap-6 px-8 pb-8">
        <div className="flex min-w-0 flex-1 flex-col">
          {listError && <p role="alert" className="mb-4 flex gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200"><IconAlert className="mt-0.5 flex-shrink-0" />{listError}</p>}
          {notice && <p role="status" className="mb-4 rounded-md border border-emerald-500/30 bg-emerald-500/10 px-4 py-2.5 text-sm text-emerald-200">{notice}</p>}
          <div className="min-h-0 flex-1 overflow-auto">
            {plans.length === 0 ? (
              <div className="flex min-h-56 flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-white/10 bg-zinc-950/40 px-6 py-14 text-center">
                <p className="text-sm font-medium text-zinc-200">{loading ? "Carregando…" : "Nenhum plano ainda"}</p>
                {!loading && <p className="max-w-md text-xs text-zinc-500">Cadastre o primeiro no formulário ao lado.</p>}
              </div>
            ) : (
              <div className="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-4">
                {plans.map((plan) => (
                  <button key={plan.id} type="button" onClick={() => edit(plan)} aria-pressed={editing === plan.id}
                    className={`flex flex-col gap-3 rounded-lg border p-4 text-left transition-colors ${editing === plan.id ? "border-cyan-400/60 bg-cyan-400/5" : "border-white/5 bg-zinc-950/60 hover:border-white/15"}`}>
                    <div className="flex items-center justify-between gap-2">
                      <span className="truncate text-sm font-semibold text-zinc-100">{plan.name}</span>
                      <Badge tone={plan.active ? "success" : "neutral"}>{plan.active ? "Ativo" : "Desativado"}</Badge>
                    </div>
                    <p className="text-2xl font-semibold text-zinc-100 tabular-nums">{plan.price_cents === 0 ? "Grátis" : brl(plan.price_cents)}<span className="ml-1 text-xs font-normal text-zinc-500">{plan.price_cents === 0 ? "" : periodShort(plan.period)}</span></p>
                    {plan.coins_per_period > 0 && <p className="text-xs text-cyan-200">{plan.coins_per_period.toLocaleString("pt-BR")} fichas {periodShort(plan.period)}</p>}
                    {plan.benefits.length > 0 && <ul className="list-disc space-y-1 pl-4 text-xs text-zinc-400">{plan.benefits.map((b) => <li key={b}>{b}</li>)}</ul>}
                    <p className="mt-auto text-[11px] text-zinc-600">Atualizado em {formatDateTime(plan.updated_at ?? undefined)}</p>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>

        <form onSubmit={(e) => void submit(e)} aria-label={editing ? "Editar plano" : "Novo plano"} className="flex w-[380px] flex-shrink-0 flex-col gap-4 self-start overflow-auto rounded-lg border border-white/5 bg-zinc-950/60 p-5">
          <p className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">{editing ? `Editar plano · ${editing}` : "Novo plano"}</p>
          <Field label="Nome"><TextInput required maxLength={40} value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} placeholder="Ex.: ShmupX Pro" /></Field>
          {!editing && <Field label="ID" hint="Fica no endereço do plano e não muda depois."><TextInput value={draft.id || slug(draft.name)} onChange={(e) => setDraft({ ...draft, id: slug(e.target.value) })} /></Field>}
          <Field label="Descrição (opcional)"><TextInput maxLength={200} value={draft.description} onChange={(e) => setDraft({ ...draft, description: e.target.value })} /></Field>
          <div className="grid grid-cols-2 gap-3">
            <Field label="Preço (R$)" hint="0 = grátis"><TextInput inputMode="decimal" value={draft.price} onChange={(e) => setDraft({ ...draft, price: e.target.value })} placeholder="19,90" /></Field>
            <Field label="Período"><Select value={draft.period} onChange={(e) => setDraft({ ...draft, period: e.target.value as PlanPeriod })}>{PERIODS.map((p) => <option key={p.value} value={p.value}>{p.label}</option>)}</Select></Field>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <Field label="Fichas por período"><TextInput type="number" min={0} max={10000} value={draft.coins} onChange={(e) => setDraft({ ...draft, coins: e.target.value })} /></Field>
            <Field label="Ordem" hint="Menor aparece antes."><TextInput type="number" value={draft.order} onChange={(e) => setDraft({ ...draft, order: e.target.value })} /></Field>
          </div>
          <Field label="Benefícios" hint="Um por linha (até 10).">
            <textarea rows={4} value={draft.benefits} onChange={(e) => setDraft({ ...draft, benefits: e.target.value })}
              className="w-full rounded-md border border-white/10 bg-zinc-950 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-cyan-400/60 focus:ring-2 focus:ring-cyan-400/15" placeholder={"Live no YouTube sem limite\nFichas extras todo mês"} />
          </Field>
          <Switch checked={draft.active} onChange={(active) => setDraft({ ...draft, active })} label="Ativo" description="Ligado, o plano é oferecido aos jogadores." />
          {error && <p role="alert" className="text-xs text-red-300">{error}</p>}
          <div className="flex gap-2">
            <Button type="submit" variant="success" loading={saving} className="flex-1">Salvar</Button>
            {editing && <Button type="button" variant={confirmDelete ? "danger" : "secondary"} onClick={() => void remove()} disabled={saving}>{confirmDelete ? "Confirmar exclusão" : "Excluir"}</Button>}
          </div>
        </form>
      </div>
    </section>
  );
}
