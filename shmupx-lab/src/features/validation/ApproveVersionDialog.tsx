import { useEffect, useRef, useState } from "react";
import Button from "../../components/ui/Button";
import { IconAlert, IconCheck, IconClose } from "../../components/ui/Icons";
import { approveScriptVersion, type ApprovedScript } from "../../lib/validation";

interface ApproveVersionDialogProps {
  gameId: string;
  gameTitle: string;
  version: string;
  /** Resumo dos vídeos desta versão, para conferir antes de aprovar (ex.: "100,0% de acerto em 3 vídeos"). */
  summary: string | null;
  onClose: () => void;
  /** Chamado depois de aprovar, para a tela reler os scripts registrados. */
  onApproved: () => void;
}

/** Confirma e aprova uma versão do script: rascunho → aprovado, e a versão aceita pelo Client. */
export default function ApproveVersionDialog({ gameId, gameTitle, version, summary, onClose, onApproved }: ApproveVersionDialogProps) {
  const [approving, setApproving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ApprovedScript | null>(null);
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    dialogRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !approving) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, approving]);

  async function approve() {
    setApproving(true);
    setError(null);
    try {
      setResult(await approveScriptVersion(gameId, version));
      onApproved();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setApproving(false);
    }
  }

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/70 p-6" onClick={() => !approving && onClose()}>
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="aprovar-versao-titulo"
        tabIndex={-1}
        onClick={(event) => event.stopPropagation()}
        className="flex max-h-full w-full max-w-xl flex-col overflow-hidden rounded-lg border border-white/10 bg-[#0e0e11] shadow-2xl shadow-black/60 outline-none"
      >
        <header className="flex items-start justify-between gap-4 border-b border-white/5 px-6 py-4">
          <div>
            <p className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">Aprovar versão</p>
            <h2 id="aprovar-versao-titulo" className="mt-0.5 text-base font-semibold text-zinc-100">
              {gameTitle} · <span className="font-mono">{version}</span>
            </h2>
          </div>
          <button type="button" onClick={onClose} disabled={approving} aria-label="Fechar" className="rounded-md p-1.5 text-zinc-500 transition-colors hover:bg-white/5 hover:text-zinc-200">
            <IconClose />
          </button>
        </header>

        <div className="space-y-4 overflow-y-auto px-6 py-5 text-sm text-zinc-300">
          {!result ? (
            <>
              <p>Aprovada, esta versão passa a validar as runs de <strong className="text-zinc-100">{gameTitle}</strong>:</p>
              <ul className="list-disc space-y-1.5 pl-5 text-xs leading-relaxed">
                <li>o <code className="font-mono text-cyan-200">script.json</code> da {version} sai de rascunho para aprovado;</li>
                <li>o Client passa a aceitar esta versão para o jogo (<code className="font-mono text-cyan-200">scripts-aceitos.json</code>): ele deixa de ser Coming soon e libera o Ranked Mode e a live;</li>
                <li>o script entra sozinho no rascunho da próxima versão do Client, na tela Versões — os jogadores só recebem com o instalador novo.</li>
              </ul>
              {summary && <p className="rounded-md border border-white/5 bg-zinc-950/60 p-3 text-xs text-zinc-400">Testes desta versão: <strong className="text-zinc-200">{summary}</strong>.</p>}
            </>
          ) : (
            <>
              <p className="flex items-center gap-2 font-medium text-emerald-200">
                <IconCheck className="text-emerald-300" />
                Versão {result.versao} aprovada.
              </p>
              <p className="text-xs leading-relaxed text-zinc-400">
                {result.cliente_atualizado
                  ? "O Client já aceita esta versão: no app de desenvolvimento vale depois que ele recompilar; para os jogadores, na próxima versão do instalador."
                  : "A lista do Client (shmupx-client/src-tauri/scripts-aceitos.json) não está ao lado do Lab: ponha a versão nela para o Client aceitar o script."}
              </p>
            </>
          )}
          {error && (
            <p role="alert" className="flex gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-200">
              <IconAlert className="mt-px flex-shrink-0" />
              {error}
            </p>
          )}
        </div>

        <footer className="flex justify-end gap-3 border-t border-white/5 px-6 py-3">
          {result ? (
            <Button variant="primary" onClick={onClose}>Concluir</Button>
          ) : (
            <>
              <Button variant="info" onClick={onClose} disabled={approving}>Cancelar</Button>
              <Button variant="success" loading={approving} onClick={() => void approve()}>Aprovar versão</Button>
            </>
          )}
        </footer>
      </div>
    </div>
  );
}
