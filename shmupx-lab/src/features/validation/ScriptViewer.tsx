import { useEffect, useRef, useState } from "react";
import Badge from "../../components/ui/Badge";
import CopyButton from "../../components/ui/CopyButton";
import { IconAlert, IconClose } from "../../components/ui/Icons";
import { getScriptSource, type ScriptInfo, type ScriptSourceFile } from "../../lib/validation";

interface ScriptViewerProps {
  gameId: string;
  script: ScriptInfo;
  onClose: () => void;
}

/** Visualizador só de leitura dos arquivos de uma versão do script: uma aba por arquivo, com números de linha. */
export default function ScriptViewer({ gameId, script, onClose }: ScriptViewerProps) {
  const [files, setFiles] = useState<ScriptSourceFile[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [active, setActive] = useState(0);
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let alive = true;
    getScriptSource(gameId, script.version)
      .then((result) => alive && setFiles(result))
      .catch((e) => alive && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      alive = false;
    };
  }, [gameId, script.version]);

  useEffect(() => {
    dialogRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const file = files?.[active] ?? null;
  const lines = file ? file.conteudo.replace(/\n$/, "").split("\n") : [];

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/70 p-6" onClick={onClose}>
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label={`Script de validação de ${gameId}`}
        tabIndex={-1}
        onClick={(event) => event.stopPropagation()}
        className="flex h-full max-h-[900px] w-full max-w-6xl flex-col overflow-hidden rounded-lg border border-white/10 bg-[#0e0e11] shadow-2xl shadow-black/60 outline-none"
      >
        <header className="flex items-start justify-between gap-4 border-b border-white/5 px-6 py-4">
          <div>
            <p className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">Script de validação · só leitura</p>
            <h2 className="mt-0.5 flex items-center gap-2 text-base font-semibold text-zinc-100">
              {script.name}
              <Badge tone="info">{script.id} {script.version}</Badge>
              {script.status === "draft" && <Badge tone="warning">rascunho</Badge>}
            </h2>
          </div>
          <button type="button" onClick={onClose} aria-label="Fechar" className="rounded-md p-1.5 text-zinc-500 transition-colors hover:bg-white/5 hover:text-zinc-200">
            <IconClose />
          </button>
        </header>

        {error && (
          <p role="alert" className="m-6 flex gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
            <IconAlert className="mt-0.5 flex-shrink-0" />
            {error}
          </p>
        )}

        {!files && !error && <p className="px-6 py-8 text-sm text-zinc-500" aria-busy="true">Carregando o script…</p>}

        {files && (
          <>
            <div role="tablist" aria-label="Arquivos do validador" className="flex gap-1 border-b border-white/5 px-4">
              {files.map((item, index) => (
                <button
                  key={item.caminho}
                  type="button"
                  role="tab"
                  aria-selected={index === active}
                  onClick={() => setActive(index)}
                  className={`border-b-2 px-3 py-2.5 font-mono text-xs transition-colors ${index === active ? "border-cyan-400 text-cyan-200" : "border-transparent text-zinc-400 hover:text-zinc-200"}`}
                >
                  {item.nome}
                </button>
              ))}
            </div>
            {file && (
              <div className="flex items-center gap-3 border-b border-white/5 px-6 py-2 text-xs text-zinc-500">
                <span className="min-w-0 flex-1 truncate font-mono" title={file.caminho}>shmupx-lab/validators/{file.caminho}</span>
                <span className="tabular-nums">{lines.length} linhas</span>
                {file.truncado && <Badge tone="warning">cortado no limite de 512 KB</Badge>}
                <CopyButton value={file.conteudo} label="Copiar o conteúdo do arquivo" />
              </div>
            )}
            <div className="min-h-0 flex-1 overflow-auto bg-black/30" role="tabpanel">
              <pre className="grid grid-cols-[auto_1fr] font-mono text-[12px] leading-relaxed">
                <code aria-hidden="true" className="sticky left-0 border-r border-white/5 bg-[#0e0e11] py-3 pr-3 pl-4 text-right text-zinc-600 select-none">
                  {lines.map((_, index) => `${index + 1}\n`).join("")}
                </code>
                <code className="py-3 pr-6 pl-4 whitespace-pre text-zinc-200">{lines.join("\n")}</code>
              </pre>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
