import { useEffect, useRef, useState } from "react";
import Button from "../../components/ui/Button";
import { IconAlert, IconCheck, IconClose } from "../../components/ui/Icons";
import { generateScript, type GeneratedScript } from "../../lib/validation";

interface GenerateScriptDialogProps {
  gameId: string;
  gameTitle: string;
  onClose: () => void;
  /** Chamado depois de gerar, para a tela reler os scripts registrados. */
  onGenerated: () => void;
}

/** Confirma e cria o primeiro script do jogo (0.1.0): manifesto, calibração e pastas das imagens. */
export default function GenerateScriptDialog({ gameId, gameTitle, onClose, onGenerated }: GenerateScriptDialogProps) {
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<GeneratedScript | null>(null);
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    dialogRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !generating) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, generating]);

  async function generate() {
    setGenerating(true);
    setError(null);
    try {
      setResult(await generateScript(gameId, gameTitle));
      onGenerated();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setGenerating(false);
    }
  }

  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/70 p-6" onClick={() => !generating && onClose()}>
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="gerar-script-titulo"
        tabIndex={-1}
        onClick={(event) => event.stopPropagation()}
        className="flex max-h-full w-full max-w-xl flex-col overflow-hidden rounded-lg border border-white/10 bg-[#0e0e11] shadow-2xl shadow-black/60 outline-none"
      >
        <header className="flex items-start justify-between gap-4 border-b border-white/5 px-6 py-4">
          <div>
            <p className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">Gerar script</p>
            <h2 id="gerar-script-titulo" className="mt-0.5 text-base font-semibold text-zinc-100">{gameTitle}</h2>
          </div>
          <button type="button" onClick={onClose} disabled={generating} aria-label="Fechar" className="rounded-md p-1.5 text-zinc-500 transition-colors hover:bg-white/5 hover:text-zinc-200">
            <IconClose />
          </button>
        </header>

        <div className="space-y-4 overflow-y-auto px-6 py-5 text-sm text-zinc-300">
          {!result ? (
            <>
              <p>
                A lógica de análise é genérica (vídeo, HUD, dígitos, vidas, continues e attempts). O que é de cada jogo é a{" "}
                <strong className="text-zinc-100">calibração</strong> e as <strong className="text-zinc-100">imagens dos dígitos</strong>. Cada versão do script é uma pasta; isto cria a primeira em{" "}
                <code className="font-mono text-xs text-zinc-200">shmupx-lab/validators/games/{gameId}/0.1.0/</code>:
              </p>
              <ul className="space-y-1.5 text-xs">
                <li><code className="font-mono text-cyan-200">script.json</code> — nome, versão <strong>0.1.0</strong>, status rascunho e o analisador genérico</li>
                <li><code className="font-mono text-cyan-200">config.json</code> — a calibração do Earthion como ponto de partida</li>
                <li><code className="font-mono text-cyan-200">templates/</code> — as pastas das imagens e o passo a passo do recorte</li>
              </ul>
              <p className="rounded-md border border-white/5 bg-zinc-950/60 p-3 text-xs leading-relaxed text-zinc-400">
                Até você ajustar a posição do placar na calibração e recortar os dígitos deste jogo (com <code className="font-mono">tools/make_digit_templates.py</code>), a leitura não funciona.
                O rascunho não chega aos jogadores: o Client só aceita validadores aprovados. Nada existente é sobrescrito.
              </p>
            </>
          ) : (
            <>
              <p className="flex items-center gap-2 font-medium text-emerald-200">
                <IconCheck className="text-emerald-300" />
                Rascunho {result.versao} criado.
              </p>
              <div>
                <p className="text-xs text-zinc-500">Criado em shmupx-lab/validators:</p>
                <ul className="mt-1 space-y-1 font-mono text-xs text-zinc-200">
                  {result.criados.map((item) => <li key={item}>{item}</li>)}
                </ul>
              </div>
              <div>
                <p className="text-xs text-zinc-500">Próximos passos:</p>
                <ol className="mt-1 list-decimal space-y-1 pl-5 text-xs leading-relaxed">
                  <li>Ajuste no <code className="font-mono">config.json</code> os rótulos do HUD e a posição do placar e das vidas (veja em <strong>Ver script</strong>).</li>
                  <li>Recorte os dígitos com <code className="font-mono">tools/make_digit_templates.py VIDEO.mp4 --game {gameId} --version {result.versao}</code> (passo a passo em <code className="font-mono">templates/LEIA-ME.md</code>).</li>
                  <li>Adicione vídeos com gabarito (de preferência com continue ou game over) e rode o script aqui até o acerto ficar bom.</li>
                </ol>
              </div>
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
              <Button variant="info" onClick={onClose} disabled={generating}>Cancelar</Button>
              <Button variant="primary" loading={generating} onClick={() => void generate()}>Gerar rascunho</Button>
            </>
          )}
        </footer>
      </div>
    </div>
  );
}
