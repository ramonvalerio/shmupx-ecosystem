import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import Badge from "../../components/ui/Badge";
import Button from "../../components/ui/Button";
import { Field, TextInput } from "../../components/ui/Field";
import { IconAlert, IconBell, IconClose } from "../../components/ui/Icons";
import Switch from "../../components/ui/Switch";
import { cancelRelease, saveRelease, scheduleRelease, type ClientRelease, type ReleaseLead } from "../../lib/backend";
import type { ScriptInfo } from "../../lib/validation";
import { getAmbiente } from "../../auth/firebase";
import { emailLabel, formatDateTime, fromLines, LEADS, noticePreview, STATUS_LABEL, STATUS_TONE, toLines } from "./options";

interface ReleaseEditorProps {
  /** Versão em edição; `null` cria uma nova (com a versão sugerida). */
  release: ClientRelease | null;
  suggestedVersion: string;
  /** Scripts de validação da pasta `validators` (para escolher os incluídos). */
  scripts: ScriptInfo[];
  onClose: () => void;
  onSaved: (release: ClientRelease, notice: string) => void;
}

const VERSION_PATTERN = /^(0|[1-9]\d{0,3})\.(0|[1-9]\d{0,3})\.(0|[1-9]\d{0,3})$/;

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h3 className="mb-3 text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">{title}</h3>
      {children}
    </section>
  );
}

function Lines({ label, hint, value, onChange, disabled }: { label: string; hint: string; value: string; onChange: (v: string) => void; disabled: boolean }) {
  return (
    <Field label={label} hint={hint}>
      <textarea
        value={value}
        disabled={disabled}
        rows={4}
        onChange={(e) => onChange(e.target.value)}
        className="w-full resize-y rounded-md border border-white/10 bg-zinc-950 px-3 py-2 text-sm leading-relaxed text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-cyan-400/60 focus:ring-2 focus:ring-cyan-400/15 disabled:cursor-not-allowed disabled:text-zinc-500"
      />
    </Field>
  );
}

/** Painel lateral: rascunho da versão, agendamento com antecedência e cancelamento. */
export default function ReleaseEditor({ release, suggestedVersion, scripts, onClose, onSaved }: ReleaseEditorProps) {
  const isNew = release === null;
  const editable = isNew || release.status === "draft";
  const [version, setVersion] = useState(release?.version ?? suggestedVersion);
  const [title, setTitle] = useState(release?.title ?? "");
  const [features, setFeatures] = useState(toLines(release?.features ?? []));
  const [fixes, setFixes] = useState(toLines(release?.fixes ?? []));
  const [rules, setRules] = useState(toLines(release?.rules ?? []));
  const [included, setIncluded] = useState<string[]>(release?.scripts ?? []);
  const [mandatory, setMandatory] = useState(release?.mandatory ?? false);
  const [lead, setLead] = useState<ReleaseLead>("48h");
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, busy]);

  // Scripts que podem entrar: os aprovados de cada jogo, mais os que a versão já lista.
  const options = [...new Set([...scripts.filter((s) => s.status === "approved").map((s) => `${s.id}@${s.version}`), ...included])].sort();
  const preview = noticePreview(version, title.trim(), mandatory, lead);

  function input() {
    return { title: title.trim(), features: fromLines(features), fixes: fromLines(fixes), rules: fromLines(rules), scripts: included, mandatory };
  }

  function problem(): string | null {
    if (!VERSION_PATTERN.test(version.trim())) return "Versão no formato 1.2.3.";
    if (!title.trim()) return "Informe o título da versão.";
    const data = input();
    if (!data.features.length && !data.fixes.length && !data.rules.length && !data.scripts.length) return "Inclua ao menos uma novidade, correção, regra ou script.";
    return null;
  }

  async function run(action: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    const invalid = problem();
    if (invalid) return setError(invalid);
    void run(async () => {
      const saved = await saveRelease(version.trim(), input());
      onSaved(saved, `Rascunho da versão ${saved.version} salvo.`);
    });
  }

  function onSchedule() {
    const invalid = problem();
    if (invalid) return setError(invalid);
    void run(async () => {
      // Grava o texto atual antes de agendar: o aviso sai exatamente com o que está na tela.
      await saveRelease(version.trim(), input());
      const scheduled = await scheduleRelease(version.trim(), lead);
      onSaved(scheduled, lead === "urgent" ? `Versão ${scheduled.version} liberada e avisada.` : `Versão ${scheduled.version} agendada para ${formatDateTime(scheduled.release_at)}.`);
    });
  }

  function onCancelSchedule() {
    if (!release) return;
    void run(async () => {
      const cancelled = await cancelRelease(release.version);
      onSaved(cancelled, `Agendamento da versão ${cancelled.version} cancelado.`);
    });
  }

  const email = release ? emailLabel(release) : null;

  return (
    <aside aria-label={isNew ? "Nova versão" : `Versão ${release.version}`} className="absolute inset-y-0 right-0 z-20 flex w-full max-w-[520px] flex-col border-l border-white/10 bg-[#0e0e11] shadow-2xl shadow-black/60 xl:static xl:z-auto xl:shadow-none">
      <header className="flex items-start justify-between gap-3 border-b border-white/5 px-6 py-4">
        <div className="min-w-0">
          <p className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">{isNew ? "Nova versão do Client" : "Versão do Client"}</p>
          <h2 className="mt-0.5 flex items-center gap-2 truncate text-base font-semibold text-zinc-100">
            <span className="font-mono">{version || "—"}</span>
            {release && <Badge tone={STATUS_TONE[release.status]}>{STATUS_LABEL[release.status]}</Badge>}
            {mandatory && <Badge tone="danger">obrigatória</Badge>}
          </h2>
        </div>
        <button type="button" onClick={onClose} aria-label="Fechar" className="rounded-md p-1.5 text-zinc-500 transition-colors hover:bg-white/5 hover:text-zinc-200 focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none">
          <IconClose />
        </button>
      </header>

      <form onSubmit={onSubmit} className="flex min-h-0 flex-1 flex-col">
        <div className="flex-1 space-y-7 overflow-y-auto px-6 py-5">
          {!editable && release && (
            <div className="space-y-1 rounded-lg border border-white/5 bg-zinc-950/60 p-3 text-xs text-zinc-400">
              <p>Agendada em {formatDateTime(release.scheduled_at)} · liberação {release.lead === "urgent" ? "imediata (urgente)" : `em ${formatDateTime(release.release_at)}`}.</p>
              {email && <p>{email}</p>}
              <p className="text-zinc-500">O aviso já saiu com este texto, então a versão não pode mais ser editada.</p>
            </div>
          )}

          <Section title="Identificação">
            <div className="grid grid-cols-3 gap-4">
              <Field label="Versão" hint="Formato 1.2.3." className="col-span-1">
                <TextInput value={version} disabled={!isNew} className="font-mono" onChange={(e) => setVersion(e.target.value.trim())} />
              </Field>
              <Field label="Título" className="col-span-2">
                <TextInput value={title} disabled={!editable} maxLength={80} autoFocus={isNew} onChange={(e) => setTitle(e.target.value)} placeholder="ex.: Sala do Aero Fighters 2" />
              </Field>
            </div>
          </Section>

          <Section title="O que muda">
            <div className="space-y-4">
              <Lines label="Novidades" hint="Uma por linha." value={features} onChange={setFeatures} disabled={!editable} />
              <Lines label="Correções" hint="Uma por linha." value={fixes} onChange={setFixes} disabled={!editable} />
              <Lines label="Mudanças de regra" hint="Uma por linha (tempo limite, revisão, ranking…)." value={rules} onChange={setRules} disabled={!editable} />
            </div>
          </Section>

          <Section title="Scripts de validação incluídos">
            {options.length === 0 ? (
              <p className="text-xs text-zinc-500">Nenhum script aprovado em validators/games. Aprove uma versão (status approved) para incluí-la aqui.</p>
            ) : (
              <div className="flex flex-wrap gap-2" role="group" aria-label="Scripts incluídos">
                {options.map((script) => {
                  const on = included.includes(script);
                  return (
                    <button
                      key={script}
                      type="button"
                      disabled={!editable}
                      aria-pressed={on}
                      onClick={() => setIncluded(on ? included.filter((s) => s !== script) : [...included, script])}
                      className={`rounded-full px-3 py-1 font-mono text-xs ring-1 transition-colors focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none disabled:cursor-not-allowed ${
                        on ? "bg-cyan-400/15 text-cyan-200 ring-cyan-400/40" : "text-zinc-400 ring-white/10 hover:text-zinc-200 hover:ring-white/20"
                      }`}
                    >
                      {script}
                    </button>
                  );
                })}
              </div>
            )}
          </Section>

          <Switch
            checked={mandatory}
            onChange={setMandatory}
            disabled={!editable}
            label="Atualização obrigatória"
            description="Depois da liberação, Clients mais antigos não abrem novas runs até atualizar (use para scripts de validação e regras novas)."
          />

          {editable && (
            <Section title="Aviso e liberação">
              <div className="space-y-3">
                <div role="radiogroup" aria-label="Antecedência" className="grid grid-cols-2 gap-2">
                  {LEADS.map((option) => (
                    <button
                      key={option.value}
                      type="button"
                      role="radio"
                      aria-checked={lead === option.value}
                      onClick={() => setLead(option.value)}
                      className={`rounded-md border px-3 py-2 text-left transition-colors ${lead === option.value ? "border-cyan-400/50 bg-cyan-400/10" : "border-white/10 hover:border-white/20"}`}
                    >
                      <span className={`block text-sm font-medium ${option.value === "urgent" ? "text-amber-200" : "text-zinc-100"}`}>{option.label}</span>
                      <span className="block text-[11px] text-zinc-500">{option.hint}</span>
                    </button>
                  ))}
                </div>
                <div className="rounded-lg border border-white/5 bg-zinc-950/60 p-3">
                  <p className="flex items-center gap-1.5 text-[11px] font-semibold tracking-wider text-zinc-500 uppercase"><IconBell width={12} height={12} /> Prévia do aviso</p>
                  <p className="mt-1.5 text-sm font-medium text-zinc-100">{preview.title}</p>
                  <p className="text-xs text-zinc-400">{preview.summary}</p>
                  <p className="mt-2 text-[11px] text-zinc-500">Vai para o sininho do Client e, por e-mail, para os jogadores que aceitam avisos.</p>
                </div>
                {confirming ? (
                  <div className="flex items-center gap-2 rounded-md border border-amber-400/20 bg-amber-500/5 p-3">
                    <IconAlert className="flex-shrink-0 text-amber-300" />
                    <p className="flex-1 text-xs text-amber-100">
                      {getAmbiente() === "prd" ? <strong>PRODUÇÃO: </strong> : "Desenvolvimento (emulador): "}
                      {getAmbiente() === "prd" ? "os jogadores reais serão avisados agora, no Client e por e-mail" : "só o ambiente local recebe o aviso"}
                      {lead === "urgent" ? " e a versão é liberada já" : ""}. Confirmar?
                    </p>
                    <Button size="sm" variant="info" onClick={() => setConfirming(false)} disabled={busy}>Cancelar</Button>
                    <Button size="sm" variant="primary" loading={busy} onClick={onSchedule}>Agendar e avisar</Button>
                  </div>
                ) : (
                  <Button variant="primary" className="w-full" onClick={() => setConfirming(true)} disabled={busy}>
                    <IconBell /> Agendar e avisar
                  </Button>
                )}
              </div>
            </Section>
          )}
        </div>

        <footer className="flex items-center gap-3 border-t border-white/5 bg-[#0e0e11] px-6 py-3">
          <p role={error ? "alert" : undefined} className="min-w-0 flex-1 text-xs text-red-300">{error}</p>
          {release?.status === "scheduled" && (
            <Button variant="danger" onClick={onCancelSchedule} loading={busy}>Cancelar agendamento</Button>
          )}
          <Button variant="info" onClick={onClose} disabled={busy}>{editable ? "Cancelar" : "Fechar"}</Button>
          {editable && <Button variant="success" type="submit" loading={busy}>Salvar rascunho</Button>}
        </footer>
      </form>
    </aside>
  );
}
