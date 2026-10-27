import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import Button from "../../components/ui/Button";
import { Field, TextInput } from "../../components/ui/Field";
import { IconAlert, IconClose } from "../../components/ui/Icons";
import { formatScore, type Expected, type ValidationVideo } from "../../lib/validation";
import VideoStatus from "./VideoStatus";
import { runnable } from "./VideoTable";

interface VideoPanelProps {
  video: ValidationVideo;
  canRun: boolean;
  onClose: () => void;
  onRun: () => void;
  onRemove: () => Promise<void>;
  onSaveExpected: (expected: Expected) => Promise<void>;
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h3 className="mb-3 text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">{title}</h3>
      {children}
    </section>
  );
}

function Fact({ label, value, alert = false }: { label: string; value: ReactNode; alert?: boolean }) {
  return (
    <div>
      <dt className="text-xs text-zinc-500">{label}</dt>
      <dd className={`mt-0.5 font-mono text-sm tabular-nums ${alert ? "text-red-300" : "text-zinc-100"}`}>{value}</dd>
    </div>
  );
}

const toText = (value: number | null) => (value === null ? "" : String(value));

/** `""` = não informado; número inteiro >= 0; qualquer outra coisa é inválida (`undefined`). */
function parse(text: string): number | null | undefined {
  const clean = text.replace(/[.\s]/g, "");
  if (clean === "") return null;
  const value = Number(clean);
  return Number.isInteger(value) && value >= 0 ? value : undefined;
}

/** Detalhes de um vídeo: gabarito completo, resultado do script e relatório bruto. */
export default function VideoPanel({ video, canRun, onClose, onRun, onRemove, onSaveExpected }: VideoPanelProps) {
  const [score, setScore] = useState(toText(video.gabarito.placar));
  const [lives, setLives] = useState(toText(video.gabarito.vidas));
  const [continues, setContinues] = useState(toText(video.gabarito.continues));
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);

  // O gabarito também muda pela tabela: acompanha o que está gravado.
  useEffect(() => setScore(toText(video.gabarito.placar)), [video.gabarito.placar]);
  useEffect(() => setLives(toText(video.gabarito.vidas)), [video.gabarito.vidas]);
  useEffect(() => setContinues(toText(video.gabarito.continues)), [video.gabarito.continues]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  async function save(event: FormEvent) {
    event.preventDefault();
    const expected = { placar: parse(score), vidas: parse(lives), continues: parse(continues) };
    if (expected.placar === undefined || expected.vidas === undefined || expected.continues === undefined) {
      return setError("Use números inteiros a partir de 0, ou deixe em branco.");
    }
    setSaving(true);
    setError(null);
    try {
      await onSaveExpected({ placar: expected.placar, vidas: expected.vidas, continues: expected.continues });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  const result = video.resultado;
  const mismatch = video.acertou === false && result && !result.erro;

  return (
    <aside
      aria-label={`Detalhes de ${video.titulo}`}
      className="absolute inset-y-0 right-0 z-20 flex w-full max-w-[440px] flex-col border-l border-white/10 bg-[#0e0e11] shadow-2xl shadow-black/60 xl:static xl:z-auto xl:shadow-none"
    >
      <header className="flex items-start justify-between gap-3 border-b border-white/5 px-6 py-4">
        <div className="min-w-0">
          <p className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">{video.origem.tipo === "youtube" ? "YouTube" : "Arquivo local"}</p>
          <h2 className="mt-0.5 line-clamp-2 text-base font-semibold break-words text-zinc-100">{video.titulo}</h2>
          <div className="mt-2"><VideoStatus video={video} /></div>
        </div>
        <button type="button" onClick={onClose} aria-label="Fechar" className="rounded-md p-1.5 text-zinc-500 transition-colors hover:bg-white/5 hover:text-zinc-200">
          <IconClose />
        </button>
      </header>

      <div className="flex-1 space-y-7 overflow-y-auto px-6 py-5">
        <Section title="Gabarito">
          <form onSubmit={save} className="space-y-3">
            <div className="grid grid-cols-3 gap-3">
              <Field label="Placar" className="col-span-3">
                <TextInput inputMode="numeric" className="font-mono" value={score} onChange={(e) => setScore(e.target.value)} placeholder="placar real da gameplay" />
              </Field>
              <Field label="Vidas perdidas">
                <TextInput inputMode="numeric" value={lives} onChange={(e) => setLives(e.target.value)} placeholder="opcional" />
              </Field>
              <Field label="Continues">
                <TextInput inputMode="numeric" value={continues} onChange={(e) => setContinues(e.target.value)} placeholder="opcional" />
              </Field>
              <div className="flex items-end">
                <Button type="submit" loading={saving} className="w-full">Salvar</Button>
              </div>
            </div>
            <p className="text-xs text-zinc-500">Vidas e continues, se informados, também precisam bater para contar como acerto.</p>
            {error && <p role="alert" className="text-xs text-red-300">{error}</p>}
          </form>
        </Section>

        <Section title="Resultado do script">
          {!result ? (
            <p className="text-sm text-zinc-500">Ainda não analisado.</p>
          ) : result.erro ? (
            <p role="alert" className="flex gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs break-words text-red-200">
              <IconAlert className="mt-px flex-shrink-0" />
              {result.erro}
            </p>
          ) : (
            <div className="space-y-4">
              <dl className="grid grid-cols-2 gap-x-4 gap-y-3">
                <Fact label="Placar lido" value={formatScore(result.placar)} alert={Boolean(mismatch)} />
                <Fact label="Esperado" value={formatScore(video.gabarito.placar)} />
                <Fact label="Vidas perdidas" value={result.vidas ?? "—"} />
                <Fact label="Continues" value={result.continues ?? "—"} />
                <Fact label="Confiança" value={result.confianca || "—"} />
                <Fact label="Game over" value={result.game_over ? "sim" : "não"} />
                <Fact label="Script" value={result.versao_validador ?? "—"} />
                <Fact label="Duração" value={`${result.duracao_segundos} s`} />
              </dl>
              <p className="rounded-md border border-white/5 bg-zinc-950/60 p-3 text-xs leading-relaxed text-zinc-400">
                {result.aceito_automatico
                  ? "O Client aceitaria esta run sem revisão."
                  : "O Client mandaria esta run para revisão (confiança baixa, pedido de revisão, game over ou continue)."}
                {result.observacao && <span className="mt-1 block text-zinc-300">{result.observacao}</span>}
              </p>
              {result.relatorio !== null && (
                <details className="group">
                  <summary className="cursor-pointer text-xs text-zinc-400 hover:text-zinc-200">Relatório completo (JSON)</summary>
                  <pre className="mt-2 max-h-80 overflow-auto rounded-md border border-white/5 bg-black/40 p-3 text-[11px] leading-relaxed text-emerald-300">
                    {JSON.stringify(result.relatorio, null, 2)}
                  </pre>
                </details>
              )}
            </div>
          )}
        </Section>

        <Section title="Origem">
          <dl className="space-y-2 text-xs">
            {video.origem.tipo === "youtube" && (
              <div>
                <dt className="text-zinc-500">Link</dt>
                <dd className="mt-0.5 break-all text-zinc-300 select-all">{video.origem.url}</dd>
              </div>
            )}
            <div>
              <dt className="text-zinc-500">Arquivo</dt>
              <dd className="mt-0.5 break-all text-zinc-300 select-all">{video.caminho ?? "ainda não disponível"}</dd>
            </div>
            {video.estado.fase === "falhou" && (
              <div>
                <dt className="text-zinc-500">Erro</dt>
                <dd className="mt-0.5 break-words text-red-300">{video.estado.erro}</dd>
              </div>
            )}
          </dl>
        </Section>
      </div>

      <footer className="flex items-center gap-3 border-t border-white/5 px-6 py-3">
        {confirmRemove ? (
          <>
            <p className="flex-1 text-xs text-zinc-400">{video.origem.tipo === "youtube" ? "Remover e apagar o vídeo baixado?" : "Remover da lista? O arquivo original fica intacto."}</p>
            <Button variant="ghost" onClick={() => setConfirmRemove(false)}>Não</Button>
            <Button className="border-red-400/40 text-red-200 hover:bg-red-500/10" onClick={() => void onRemove()}>Remover</Button>
          </>
        ) : (
          <>
            <Button variant="ghost" onClick={() => setConfirmRemove(true)} disabled={video.estado.fase === "analisando"} className="mr-auto">
              Remover
            </Button>
            <Button variant="primary" onClick={onRun} disabled={!canRun || !runnable(video)}>
              {video.estado.fase === "concluido" ? "Rodar de novo" : "Rodar script"}
            </Button>
          </>
        )}
      </footer>
    </aside>
  );
}
