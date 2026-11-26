import { useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";
import Badge from "../../components/ui/Badge";
import Button from "../../components/ui/Button";
import { Field, TextInput } from "../../components/ui/Field";
import { IconAlert, IconBell, IconClose } from "../../components/ui/Icons";
import Switch from "../../components/ui/Switch";
import {
  cancelRelease,
  removeAutoItem,
  saveRelease,
  scheduleRelease,
  setReleaseInstaller,
  setReleasePortable,
  type ClientRelease,
  type ReleaseLead,
  type ReleaseSection,
} from "../../lib/backend";
import { buildInstaller, formatSize, getClientNotes, getLocalInstaller, onInstallerBuild, publishInstaller, type LocalInstaller } from "../../lib/installer";
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
  /** Gravação silenciosa (salvamento automático, item retirado): só atualiza a lista. */
  onChanged: (release: ClientRelease) => void;
}

const SECTION_LABEL: Record<ReleaseSection, string> = { features: "Novidade", fixes: "Correção", rules: "Regra", scripts: "Script" };

/** Espera depois da última tecla antes de salvar o rascunho sozinho. */
const AUTOSAVE_MS = 1200;

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
export default function ReleaseEditor({ release, suggestedVersion, scripts, onClose, onSaved, onChanged }: ReleaseEditorProps) {
  const isNew = release === null;
  const editable = isNew || release.status === "draft";
  const autoItems = release?.auto ?? [];
  const [autoUpdate, setAutoUpdate] = useState(Boolean(release?.installer));
  const [sendEmail, setSendEmail] = useState(true);
  const [local, setLocal] = useState<LocalInstaller | null>(null);
  // Build do instalador pelo Lab: as últimas linhas da saída, o erro e quando começou.
  const [build, setBuild] = useState<{ running: boolean; lines: string[]; error: string | null; startedAt: number } | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [autosave, setAutosave] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [savedAt, setSavedAt] = useState<Date | null>(null);
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
  const autoScripts = autoItems.filter((i) => i.section === "scripts").map((i) => i.text);
  const options = [...new Set([...scripts.filter((s) => s.status === "approved").map((s) => `${s.id}@${s.version}`), ...included])]
    .filter((s) => !autoScripts.includes(s))
    .sort();
  const preview = noticePreview(version, title.trim(), mandatory, lead);
  const hasScripts = included.length > 0 || autoScripts.length > 0;

  function input() {
    return { title: title.trim(), features: fromLines(features), fixes: fromLines(fixes), rules: fromLines(rules), scripts: included, mandatory };
  }

  function problem(): string | null {
    if (!VERSION_PATTERN.test(version.trim())) return "Versão no formato 1.2.3.";
    if (!title.trim()) return "Informe o título da versão.";
    const data = input();
    if (!data.features.length && !data.fixes.length && !data.rules.length && !data.scripts.length && autoItems.length === 0) return "Inclua ao menos uma novidade, correção, regra ou script.";
    return null;
  }

  // Rascunho existente: o texto manual se salva sozinho, sem botão.
  const firstRender = useRef(true);
  useEffect(() => {
    if (firstRender.current) {
      firstRender.current = false;
      return;
    }
    if (isNew || !editable || !title.trim()) return;
    setAutosave("saving");
    const timer = window.setTimeout(() => {
      saveRelease(version.trim(), input())
        .then((saved) => {
          onChanged(saved);
          setAutosave("saved");
          setSavedAt(new Date());
        })
        .catch(() => setAutosave("error"));
    }, AUTOSAVE_MS);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- salva quando o conteúdo muda
  }, [title, features, fixes, rules, included, mandatory]);

  // Notas pelos commits do Client: preenche sozinho um rascunho vazio; o botão junta o que faltar.
  const [notesInfo, setNotesInfo] = useState<string | null>(null);
  const [notesBusy, setNotesBusy] = useState(false);
  async function fillFromCommits(onlyIfEmpty: boolean) {
    if (!editable) return;
    setNotesBusy(true);
    try {
      const notes = await getClientNotes(version.trim());
      if (onlyIfEmpty && (fromLines(features).length || fromLines(fixes).length || fromLines(rules).length)) return;
      // Gerar de novo substitui novidades e correções (as regras são sempre escritas à mão).
      setFeatures(toLines(notes.features));
      setFixes(toLines(notes.fixes));
      if (!title.trim()) setTitle(`Versão ${version.trim()}`);
      const total = notes.features.length + notes.fixes.length;
      setNotesInfo(
        !notes.rewritten && total > 0
          ? `Texto dos commits, sem a reescrita para o jogador (${notes.rewrite_error ?? "IA indisponível"}). Revise antes de agendar.`
          : total === 0
          ? `Nenhuma novidade ou correção nos commits do Client${notes.since ? ` desde ${notes.since}` : ""}.`
          : `Resumo para o jogador de ${notes.commits} ${notes.commits === 1 ? "alteração" : "alterações"} do Client. Revise e edite à vontade.`,
      );
    } catch (e) {
      setNotesInfo(`Não foi possível ler os commits do Client: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setNotesBusy(false);
    }
  }
  useEffect(() => {
    if (!isNew && editable) void fillFromCommits(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- só ao abrir o rascunho
  }, [release?.version]);

  // Build local do Client para esta versão (o que "Enviar instalador" vai publicar).
  useEffect(() => {
    if (isNew || !editable) return;
    getLocalInstaller(version.trim()).then(setLocal).catch(() => setLocal(null));
  }, [isNew, editable, version]);

  // Andamento do build: cada linha da saída; no fim, relê o build local para liberar o envio.
  useEffect(() => {
    const target = version.trim();
    const unlisten = onInstallerBuild((event) => {
      if (event.versao !== target) return;
      if (event.fase === "rodando" && event.linha) {
        const line = event.linha;
        setBuild((current) => current && { ...current, running: true, lines: [...current.lines, line].slice(-6) });
        return;
      }
      setBuild((current) => current && { ...current, running: false, error: event.fase === "falhou" ? event.erro ?? "o build falhou" : null });
      getLocalInstaller(target).then(setLocal).catch(() => setLocal(null));
    });
    return () => { void unlisten.then((stop) => stop()); };
  }, [version]);
  useEffect(() => {
    if (!build?.running) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [build?.running]);

  async function onBuildInstaller() {
    const target = version.trim();
    setError(null);
    setBuild({ running: true, lines: [], error: null, startedAt: Date.now() });
    setNow(Date.now());
    try {
      await buildInstaller(target);
    } catch (e) {
      setBuild((current) => current && { ...current, running: false, error: e instanceof Error ? e.message : String(e) });
    }
  }

  function onRemoveAuto(key: string) {
    if (!release) return;
    void run(async () => onChanged(await removeAutoItem(release.version, key)));
  }

  function onPublishInstaller() {
    if (!release) return;
    void run(async () => {
      const { portable, ...installer } = await publishInstaller(release.version);
      await setReleaseInstaller(release.version, installer);
      const saved = await setReleasePortable(release.version, portable);
      setAutoUpdate(true);
      onSaved(saved, `Instalador (${formatSize(installer.size)}) e ShmupX Portable (${formatSize(portable.size)}) da versão ${saved.version} publicados.`);
    });
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
      const scheduled = await scheduleRelease(version.trim(), lead, { autoUpdate: autoUpdate && Boolean(release?.installer), sendEmail });
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

          {editable && !isNew && (
            <Section title="Gerado pelas alterações no Lab">
              {autoItems.length === 0 ? (
                <p className="text-xs text-zinc-500">
                  Nada ainda. Ao publicar ou alterar um jogo no Catálogo, ou aprovar um script de validação, a linha correspondente aparece aqui sozinha.
                </p>
              ) : (
                <ul className="space-y-1.5">
                  {autoItems.map((item) => (
                    <li key={item.key} className="flex items-start gap-2 rounded-md border border-white/5 bg-zinc-950/60 px-3 py-2">
                      <span className="mt-0.5 w-20 flex-shrink-0 text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">{SECTION_LABEL[item.section]}</span>
                      <span className={`min-w-0 flex-1 text-sm text-zinc-200 ${item.section === "scripts" ? "font-mono" : ""}`}>{item.text}</span>
                      <button
                        type="button"
                        onClick={() => onRemoveAuto(item.key)}
                        disabled={busy}
                        aria-label={`Tirar "${item.text}" da versão`}
                        title="Tirar da versão"
                        className="rounded p-0.5 text-zinc-500 transition-colors hover:bg-white/5 hover:text-zinc-200 focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none"
                      >
                        <IconClose width={14} height={14} />
                      </button>
                    </li>
                  ))}
                </ul>
              )}
              <p className="mt-2 text-[11px] text-zinc-500">Entram na versão ao agendar, junto com o que você escrever abaixo. Repetir a mesma alteração atualiza a linha em vez de duplicar.</p>
            </Section>
          )}

          <Section title={editable && !isNew ? "O que muda no Client" : "O que muda"}>
            <div className="space-y-4">
              {editable && !isNew && (
                <div className="flex items-start justify-between gap-3 rounded-md border border-white/5 bg-zinc-950/60 px-3 py-2">
                  <p className="min-w-0 text-xs text-zinc-400">{notesBusy ? "Lendo os commits do Client…" : notesInfo ?? "Gerado a partir das alterações do Client desde a versão anterior, em linguagem para o jogador."}</p>
                  <Button size="sm" variant="ghost" loading={notesBusy} onClick={() => void fillFromCommits(false)}>Gerar de novo</Button>
                </div>
              )}
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

          {editable && !isNew && (
            <Section title="Instalador">
              <div className="space-y-2 rounded-lg border border-white/5 bg-zinc-950/60 p-3 text-xs">
                {release?.installer ? (
                  <p className="text-emerald-300">
                    Publicado: <span className="font-mono break-all">{release.installer.url}</span> ({formatSize(release.installer.size)})
                    {release.portable && <><br />Portátil: <span className="font-mono break-all">{release.portable.url}</span> ({formatSize(release.portable.size)})</>}
                  </p>
                ) : (
                  <p className="text-zinc-400">Ainda sem instalador. {hasScripts ? "Esta versão leva scripts de validação, então precisa de um." : "Sem instalador, a versão só traz notas e não atualiza o Client."}</p>
                )}
                {local?.found ? (
                  <p className="text-zinc-500">
                    Build local: {formatSize(local.size)} · gerado em {formatDateTime(local.built_at_ms ? new Date(local.built_at_ms) : undefined)}
                    {local.signed ? " · assinado" : " · sem assinatura do updater: gere de novo (a chave vem do OpenBao, shmupx/updater)"}
                    {local.portable_found ? ` · portátil ${formatSize(local.portable_size)}` : " · sem o ShmupX Portable: gere de novo"}
                  </p>
                ) : (
                  <p className="text-zinc-500">
                    Nenhum build local da versão {version}. Gere o instalador aqui (ShmupX_{version}_x64-setup.exe) e depois envie.
                  </p>
                )}
                {build && (build.running || build.error) && (
                  <div className="space-y-1.5 rounded-md border border-white/5 bg-black/40 p-2">
                    {build.running && (
                      <p className="text-cyan-200">
                        Gerando o instalador {version}… {elapsed(now - build.startedAt)} (leva uns 10 a 20 minutos; pode seguir usando o Lab)
                      </p>
                    )}
                    {build.lines.length > 0 && build.running && (
                      <pre className="max-h-28 overflow-hidden font-mono text-[10.5px] leading-relaxed whitespace-pre-wrap text-zinc-500">{build.lines.join("\n")}</pre>
                    )}
                    {build.error && <p role="alert" className="whitespace-pre-wrap text-red-300">{build.error}</p>}
                  </div>
                )}
                <div className="flex flex-wrap gap-2">
                  <Button size="sm" variant={local?.found && local.signed ? "secondary" : "primary"} onClick={() => void onBuildInstaller()} disabled={busy || Boolean(build?.running)} loading={Boolean(build?.running)}
                    title={`Ajusta o Client para a versão ${version}, gera e assina o instalador neste PC (npm run bundle)`}>
                    {local?.found ? "Gerar de novo" : "Gerar instalador"}
                  </Button>
                  <Button size="sm" variant="info" onClick={onPublishInstaller} disabled={busy || Boolean(build?.running) || !local?.found || !local.signed || !local.portable_found} loading={busy}>
                    {release?.installer ? "Enviar de novo" : "Enviar instalador"}
                  </Button>
                </div>
              </div>
            </Section>
          )}

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
                  <p className="mt-2 text-[11px] text-zinc-500">Vai para o sininho do Client{sendEmail ? " e, por e-mail, para os jogadores que aceitam avisos" : ""}.</p>
                </div>
                <Switch
                  checked={autoUpdate && Boolean(release?.installer)}
                  onChange={setAutoUpdate}
                  disabled={!release?.installer}
                  label="Atualizar os Clients automaticamente"
                  description={release?.installer ? "Na liberação, o Client baixa a versão sozinho e pede para reiniciar." : "Envie o instalador para liberar esta opção."}
                />
                <Switch
                  checked={sendEmail}
                  onChange={setSendEmail}
                  label="Enviar e-mail aos jogadores"
                  description="Só para quem aceita avisos por e-mail. Desligado, o aviso fica só no sininho do Client."
                />
                {confirming ? (
                  <div className="flex items-center gap-2 rounded-md border border-amber-400/20 bg-amber-500/5 p-3">
                    <IconAlert className="flex-shrink-0 text-amber-300" />
                    <p className="flex-1 text-xs text-amber-100">
                      {getAmbiente() === "prd" ? <strong>PRODUÇÃO: </strong> : "Desenvolvimento (emulador): "}
                      {getAmbiente() === "prd" ? `os jogadores reais serão avisados agora, no Client${sendEmail ? " e por e-mail" : ""}` : "só o ambiente local recebe o aviso"}
                      {lead === "urgent" ? " e a versão é liberada já" : ""}
                      {autoUpdate && release?.installer ? "; os Clients se atualizam sozinhos na liberação" : ""}. Confirmar?
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
          {error ? (
            <p role="alert" className="min-w-0 flex-1 text-xs text-red-300">{error}</p>
          ) : (
            <p role="status" className="min-w-0 flex-1 text-xs text-zinc-500">
              {autosave === "saving" ? "Salvando…" : autosave === "error" ? "Não foi possível salvar sozinho; tente de novo." : savedAt ? `Salvo automaticamente às ${savedAt.toLocaleTimeString("pt-BR", { timeStyle: "short" })}.` : ""}
            </p>
          )}
          {release?.status === "scheduled" && (
            <Button variant="danger" onClick={onCancelSchedule} loading={busy}>Cancelar agendamento</Button>
          )}
          <Button variant="info" onClick={onClose} disabled={busy}>{editable && isNew ? "Cancelar" : "Fechar"}</Button>
          {editable && isNew && <Button variant="success" type="submit" loading={busy}>Salvar rascunho</Button>}
        </footer>
      </form>
    </aside>
  );
}

/** Tempo decorrido do build: `3min 05s`. */
function elapsed(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(seconds / 60)}min ${String(seconds % 60).padStart(2, "0")}s`;
}
