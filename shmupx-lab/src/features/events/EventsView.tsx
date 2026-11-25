import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";
import Badge from "../../components/ui/Badge";
import Button from "../../components/ui/Button";
import Switch from "../../components/ui/Switch";
import { Field, Select, TextInput } from "../../components/ui/Field";
import { IconAlert, IconRefresh } from "../../components/ui/Icons";
import { deleteEvent, listEvents, saveEvent, type CatalogGame, type EventMode, type LabEvent } from "../../lib/backend";
import { formatDateTime, toLocalInput } from "../coins/options";

const MODES: { value: EventMode; label: string }[] = [
  { value: "highscore", label: "Hi-Score" },
  { value: "1cc", label: "1CC" },
  { value: "caravan", label: "Caravan" },
];
const TEXTAREA = "w-full rounded-md border border-white/10 bg-zinc-950 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-cyan-400/60 focus:ring-2 focus:ring-cyan-400/15";

interface Draft { id: string; title: string; description: string; games: string[]; mode: EventMode; starts: string; ends: string; rules: string; prizes: string; published: boolean }
const empty = (): Draft => {
  const start = new Date(Date.now() + 7 * 86_400_000);
  start.setHours(20, 0, 0, 0);
  const end = new Date(start.getTime() + 30 * 86_400_000);
  end.setHours(23, 59, 0, 0);
  return { id: "", title: "", description: "", games: [], mode: "highscore", starts: toLocalInput(start), ends: toLocalInput(end), rules: "", prizes: "", published: false };
};
const toDraft = (e: LabEvent): Draft => ({
  id: e.id, title: e.title, description: e.description, games: e.game_ids, mode: e.mode,
  starts: e.starts_at ? toLocalInput(new Date(e.starts_at)) : "", ends: e.ends_at ? toLocalInput(new Date(e.ends_at)) : "",
  rules: e.rules.join("\n"), prizes: e.prizes.join("\n"), published: e.published,
});
const slug = (text: string) => text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

/** Situação do evento pelas datas. */
function phase(e: LabEvent, now: number): { label: string; tone: "info" | "success" | "neutral" } {
  const start = e.starts_at ? Date.parse(e.starts_at) : NaN;
  const end = e.ends_at ? Date.parse(e.ends_at) : NaN;
  if (now >= end) return { label: "Encerrado", tone: "neutral" };
  if (now < start) return { label: "Em breve", tone: "info" };
  return { label: "Aberto", tone: "success" };
}

/**
 * Events: eventos e torneios (`events/{id}`): datas, jogos do catálogo que valem, o modo (a tabela
 * do ranking), regras e premiação. Publicado, o evento aparece no Client.
 */
export default function EventsView({ games }: { games: CatalogGame[] }) {
  const [events, setEvents] = useState<LabEvent[]>([]);
  const [loading, setLoading] = useState(false);
  const [listError, setListError] = useState<string | null>(null);
  const [draft, setDraft] = useState<Draft>(empty);
  const [editing, setEditing] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const now = Date.now();
  const titles = useMemo(() => new Map(games.map((g) => [g.id, g.title])), [games]);

  const reload = useCallback(async () => {
    setLoading(true);
    setListError(null);
    try { setEvents(await listEvents()); } catch (e) { setListError(e instanceof Error ? e.message : String(e)); } finally { setLoading(false); }
  }, []);
  useEffect(() => { void reload(); }, [reload]);

  function edit(event: LabEvent | null) {
    setEditing(event?.id ?? null);
    setDraft(event ? toDraft(event) : empty());
    setError(null);
    setNotice(null);
    setConfirmDelete(false);
  }
  const toggleGame = (id: string) => setDraft((d) => ({ ...d, games: d.games.includes(id) ? d.games.filter((g) => g !== id) : [...d.games, id] }));
  const id = editing ?? (draft.id || slug(draft.title));

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!id) { setError("Dê um título ao evento."); return; }
    if (draft.games.length === 0) { setError("Escolha pelo menos um jogo."); return; }
    if (!draft.starts || !draft.ends || new Date(draft.ends) <= new Date(draft.starts)) { setError("O fim precisa ser depois do começo."); return; }
    setSaving(true);
    setError(null);
    try {
      const saved = await saveEvent(id, {
        title: draft.title, description: draft.description, game_ids: draft.games, mode: draft.mode,
        starts_at: new Date(draft.starts).toISOString(), ends_at: new Date(draft.ends).toISOString(),
        rules: draft.rules.split("\n"), prizes: draft.prizes.split("\n"), published: draft.published,
      });
      setEvents((current) => [...current.filter((e) => e.id !== saved.id), saved].sort((a, b) => (b.starts_at ?? "").localeCompare(a.starts_at ?? "")));
      edit(saved);
      setNotice(`Evento ${saved.title} salvo${saved.published ? " e publicado" : ""}.`);
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
      await deleteEvent(editing);
      setEvents((current) => current.filter((e) => e.id !== editing));
      edit(null);
      setNotice("Evento excluído.");
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
          <h1 className="text-xl font-semibold text-zinc-100">Events</h1>
          <p className="mt-1 max-w-2xl text-sm text-zinc-500">Eventos e torneios: datas, jogos, modo, regras e premiação. Publicado, o evento aparece no Client.</p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="secondary" size="sm" onClick={() => edit(null)}>Novo evento</Button>
          <Button variant="ghost" onClick={() => void reload()} disabled={loading} title="Recarregar" aria-label="Recarregar"><IconRefresh className={loading ? "animate-spin" : ""} /></Button>
        </div>
      </header>

      <div className="flex min-h-0 flex-1 gap-6 px-8 pb-8">
        <div className="flex min-w-0 flex-1 flex-col">
          {listError && <p role="alert" className="mb-4 flex gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200"><IconAlert className="mt-0.5 flex-shrink-0" />{listError}</p>}
          {notice && <p role="status" className="mb-4 rounded-md border border-emerald-500/30 bg-emerald-500/10 px-4 py-2.5 text-sm text-emerald-200">{notice}</p>}
          <div className="min-h-0 flex-1 overflow-auto rounded-lg border border-white/5 bg-zinc-950/40">
            {events.length === 0 ? (
              <div className="flex h-full min-h-56 flex-col items-center justify-center gap-2 px-6 py-14 text-center">
                <p className="text-sm font-medium text-zinc-200">{loading ? "Carregando…" : "Nenhum evento ainda"}</p>
                {!loading && <p className="max-w-md text-xs text-zinc-500">Cadastre o primeiro no formulário ao lado.</p>}
              </div>
            ) : (
              <table className="w-full text-left text-sm">
                <thead className="sticky top-0 bg-zinc-950 text-[11px] tracking-wider text-zinc-500 uppercase">
                  <tr><th className="px-4 py-3 font-semibold">Evento</th><th className="px-4 py-3 font-semibold">Jogos</th><th className="px-4 py-3 font-semibold">Período</th><th className="px-4 py-3 font-semibold">Situação</th></tr>
                </thead>
                <tbody>
                  {events.map((e) => {
                    const p = phase(e, now);
                    return (
                      <tr key={e.id} onClick={() => edit(e)} className={`cursor-pointer border-t border-white/5 ${editing === e.id ? "bg-cyan-400/5" : "hover:bg-white/[.02]"}`}>
                        <td className="px-4 py-3"><p className="font-medium text-zinc-100">{e.title}</p><p className="text-[11px] text-zinc-500">{MODES.find((m) => m.value === e.mode)?.label}</p></td>
                        <td className="px-4 py-3 text-xs text-zinc-400">{e.game_ids.map((g) => titles.get(g) ?? g).join(", ")}</td>
                        <td className="px-4 py-3 text-xs text-zinc-400 tabular-nums">{formatDateTime(e.starts_at ?? undefined)} → {formatDateTime(e.ends_at ?? undefined)}</td>
                        <td className="px-4 py-3"><span className="flex flex-wrap gap-1"><Badge tone={p.tone}>{p.label}</Badge>{!e.published && <Badge tone="warning">Rascunho</Badge>}</span></td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
          </div>
        </div>

        <form onSubmit={(e) => void submit(e)} aria-label={editing ? "Editar evento" : "Novo evento"} className="flex max-h-full w-[400px] flex-shrink-0 flex-col gap-4 self-start overflow-auto rounded-lg border border-white/5 bg-zinc-950/60 p-5">
          <p className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">{editing ? `Editar evento · ${editing}` : "Novo evento"}</p>
          <Field label="Título"><TextInput required maxLength={80} value={draft.title} onChange={(e) => setDraft({ ...draft, title: e.target.value })} placeholder="Ex.: Vertical Master Cup" /></Field>
          {!editing && <Field label="ID" hint="Fica no endereço do evento e não muda depois."><TextInput value={draft.id || slug(draft.title)} onChange={(e) => setDraft({ ...draft, id: slug(e.target.value) })} /></Field>}
          <Field label="Descrição (opcional)"><textarea rows={3} maxLength={600} value={draft.description} onChange={(e) => setDraft({ ...draft, description: e.target.value })} className={TEXTAREA} /></Field>
          <div className="grid grid-cols-2 gap-3">
            <Field label="Começa"><TextInput type="datetime-local" className="[color-scheme:dark]" value={draft.starts} onChange={(e) => setDraft({ ...draft, starts: e.target.value })} /></Field>
            <Field label="Termina"><TextInput type="datetime-local" className="[color-scheme:dark]" value={draft.ends} onChange={(e) => setDraft({ ...draft, ends: e.target.value })} /></Field>
          </div>
          <Field label="Modo" hint="A tabela do ranking que vale no evento."><Select value={draft.mode} onChange={(e) => setDraft({ ...draft, mode: e.target.value as EventMode })}>{MODES.map((m) => <option key={m.value} value={m.value}>{m.label}</option>)}</Select></Field>
          <div>
            <span className="mb-1.5 block text-xs font-medium text-zinc-400">Jogos ({draft.games.length})</span>
            <div className="flex max-h-40 flex-wrap gap-1.5 overflow-auto rounded-md border border-white/10 bg-zinc-950 p-2">
              {games.map((g) => (
                <button key={g.id} type="button" aria-pressed={draft.games.includes(g.id)} onClick={() => toggleGame(g.id)}
                  className={`rounded-md border px-2 py-1 text-xs transition-colors ${draft.games.includes(g.id) ? "border-cyan-400/60 bg-cyan-400/10 text-cyan-200" : "border-white/10 text-zinc-400 hover:border-white/20 hover:text-zinc-200"}`}>{g.title}</button>
              ))}
              {games.length === 0 && <span className="text-xs text-zinc-500">O catálogo está vazio.</span>}
            </div>
          </div>
          <Field label="Regras" hint="Uma por linha."><textarea rows={3} value={draft.rules} onChange={(e) => setDraft({ ...draft, rules: e.target.value })} className={TEXTAREA} placeholder={"Só runs Ranked\nVale a melhor run de cada jogador"} /></Field>
          <Field label="Premiação" hint="Uma linha por prêmio."><textarea rows={3} value={draft.prizes} onChange={(e) => setDraft({ ...draft, prizes: e.target.value })} className={TEXTAREA} placeholder={"1º lugar: 500 fichas\n2º lugar: 250 fichas"} /></Field>
          <Switch checked={draft.published} onChange={(published) => setDraft({ ...draft, published })} label="Publicado" description="Visível no Client. Desligado, o evento fica como rascunho." />
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
