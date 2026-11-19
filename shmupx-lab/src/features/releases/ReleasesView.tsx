import { useEffect, useState } from "react";
import Badge from "../../components/ui/Badge";
import Button from "../../components/ui/Button";
import { IconAlert, IconCheck, IconPlus, IconRefresh } from "../../components/ui/Icons";
import type { ClientRelease } from "../../lib/backend";
import type { ScriptInfo } from "../../lib/validation";
import { emailLabel, formatDateTime, STATUS_LABEL, STATUS_TONE } from "./options";
import ReleaseEditor from "./ReleaseEditor";
import { suggestNextVersion } from "./useReleases";

interface ReleasesViewProps {
  releases: ClientRelease[];
  loading: boolean;
  error: string | null;
  scripts: ScriptInfo[];
  onReload: () => void;
  onSaved: (release: ClientRelease) => void;
}

/** `"new"` = criando; uma versão = editando; `null` = painel fechado. */
type Editing = "new" | string | null;

/** Versões do ShmupX Client: o que muda, aviso antecipado (sininho e e-mail) e liberação. */
export default function ReleasesView({ releases, loading, error, scripts, onReload, onSaved }: ReleasesViewProps) {
  const [editing, setEditing] = useState<Editing>(null);
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 5000);
    return () => window.clearTimeout(timer);
  }, [notice]);

  const editingRelease = editing && editing !== "new" ? releases.find((r) => r.version === editing) ?? null : null;

  return (
    <div className="relative flex min-w-0 flex-1 overflow-hidden">
      <section className="flex min-w-0 flex-1 flex-col">
        <header className="flex flex-wrap items-end justify-between gap-4 px-8 pt-7 pb-5">
          <div>
            <h1 className="text-xl font-semibold text-zinc-100">Versões do Client</h1>
            <p className="mt-1 max-w-2xl text-sm text-zinc-500">
              Novidades, correções, regras e scripts de validação de cada versão. Ao agendar, os jogadores são avisados no sininho do Client e por e-mail antes da liberação; urgente libera na hora.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="ghost" onClick={onReload} disabled={loading} title="Recarregar" aria-label="Recarregar">
              <IconRefresh className={loading ? "animate-spin" : ""} />
            </Button>
            <Button variant="primary" onClick={() => setEditing("new")}>
              <IconPlus />
              Nova versão
            </Button>
          </div>
        </header>

        {error && (
          <p role="alert" className="mx-8 mb-4 flex gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
            <IconAlert className="mt-0.5 flex-shrink-0" />
            {error}
          </p>
        )}

        <div className="mx-8 mb-8 min-h-0 flex-1 overflow-auto rounded-lg border border-white/5 bg-zinc-950/40">
          {releases.length === 0 ? (
            <div className="flex h-full min-h-56 flex-col items-center justify-center gap-2 px-6 py-14 text-center">
              <p className="text-sm font-medium text-zinc-200">{loading ? "Carregando…" : "Nenhuma versão ainda"}</p>
              {!loading && <p className="max-w-md text-xs text-zinc-500">Crie a próxima versão do Client, descreva o que muda e agende o aviso.</p>}
            </div>
          ) : (
            <table className="w-full text-left text-sm">
              <thead className="sticky top-0 bg-zinc-950 text-[11px] tracking-wider text-zinc-500 uppercase">
                <tr>
                  <th className="px-4 py-3 font-semibold">Versão</th>
                  <th className="px-4 py-3 font-semibold">Título</th>
                  <th className="px-4 py-3 font-semibold">Situação</th>
                  <th className="px-4 py-3 font-semibold">Liberação</th>
                  <th className="px-4 py-3 font-semibold">Avisos</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/5">
                {releases.map((release) => (
                  <tr
                    key={release.version}
                    onClick={() => setEditing(release.version)}
                    className={`cursor-pointer transition-colors hover:bg-white/[0.03] ${editing === release.version ? "bg-white/[0.04]" : ""}`}
                  >
                    <td className="px-4 py-3 font-mono text-zinc-100">{release.version}</td>
                    <td className="px-4 py-3 text-zinc-200">
                      {release.title}
                      {release.scripts.length > 0 && <span className="ml-2 text-xs text-zinc-500">{release.scripts.length} script(s)</span>}
                    </td>
                    <td className="px-4 py-3">
                      <span className="flex flex-wrap gap-1">
                        <Badge tone={STATUS_TONE[release.status]}>{STATUS_LABEL[release.status]}</Badge>
                        {release.mandatory && <Badge tone="danger">obrigatória</Badge>}
                        {release.lead === "urgent" && <Badge tone="warning">urgente</Badge>}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-xs text-zinc-400 tabular-nums">{release.status === "draft" ? "—" : formatDateTime(release.release_at)}</td>
                    <td className="px-4 py-3 text-xs text-zinc-500">{emailLabel(release) ?? (release.status === "draft" ? "ainda não avisada" : "—")}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </section>

      {editing !== null && (
        <ReleaseEditor
          key={editing}
          release={editingRelease}
          suggestedVersion={suggestNextVersion(releases)}
          scripts={scripts}
          onClose={() => setEditing(null)}
          onChanged={onSaved}
          onSaved={(saved, message) => {
            onSaved(saved);
            setEditing(saved.status === "draft" ? saved.version : null);
            setNotice(message);
          }}
        />
      )}

      {notice && (
        <div role="status" className="pointer-events-none fixed right-6 bottom-6 z-30 flex items-center gap-2 rounded-lg border border-emerald-400/20 bg-zinc-900 px-4 py-3 text-sm text-zinc-100 shadow-xl shadow-black/40">
          <IconCheck className="text-emerald-300" />
          {notice}
        </div>
      )}
    </div>
  );
}
