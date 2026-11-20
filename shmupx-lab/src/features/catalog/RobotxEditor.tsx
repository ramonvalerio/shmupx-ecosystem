import { useEffect, useMemo, useState } from "react";
import { getRobotx, saveRobotx, type RobotxItem } from "../../lib/backend";
import Button from "../../components/ui/Button";
import { TextInput } from "../../components/ui/Field";
import { IconClose, IconPlus } from "../../components/ui/Icons";

const MAX_QUESTION = 200;
const MAX_ANSWER = 1000;

/** Seção do editor de jogo: resumo do conhecimento do RobotX e botão para editar. */
export function RobotxField({ gameId, title, isNew }: { gameId: string; title: string; isNew: boolean }) {
  const [count, setCount] = useState<number | null>(null);
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (isNew) return;
    getRobotx(gameId).then((items) => setCount(items.length)).catch(() => setCount(null));
  }, [gameId, isNew]);

  if (isNew) return <p className="text-xs text-zinc-500">Crie o jogo primeiro; depois cadastre as perguntas e respostas do RobotX.</p>;
  return (
    <div className="flex items-center justify-between gap-3 rounded-lg border border-white/10 bg-zinc-950/60 px-4 py-3">
      <div className="min-w-0">
        <p className="text-sm text-zinc-200">{count === null ? "…" : count === 0 ? "Nenhuma pergunta cadastrada" : `${count} ${count === 1 ? "pergunta cadastrada" : "perguntas cadastradas"}`}</p>
        <p className="text-xs text-zinc-500">{count ? "O RobotX responde na sala quando mencionado (@RobotX)." : "Sem perguntas, o RobotX aparece como \"em breve\" na sala."}</p>
      </div>
      <Button size="sm" variant="primary" onClick={() => setOpen(true)}>Editar</Button>
      {open && <RobotxDialog gameId={gameId} title={title} onClose={(saved) => { setOpen(false); if (saved) setCount(saved.length); }} />}
    </div>
  );
}

function RobotxDialog({ gameId, title, onClose }: { gameId: string; title: string; onClose: (saved?: RobotxItem[]) => void }) {
  const [items, setItems] = useState<RobotxItem[] | null>(null);
  const [filter, setFilter] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    getRobotx(gameId).then((loaded) => setItems(loaded.length ? loaded : [{ question: "", answer: "" }])).catch((e) => setError(String(e instanceof Error ? e.message : e)));
  }, [gameId]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => { if (event.key === "Escape" && !busy) { event.stopPropagation(); close(); } };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  });

  function close() {
    if (dirty && !window.confirm("Descartar as alterações do RobotX?")) return;
    onClose();
  }
  const update = (index: number, changes: Partial<RobotxItem>) => {
    setDirty(true);
    setItems((current) => current?.map((item, i) => (i === index ? { ...item, ...changes } : item)) ?? null);
  };
  const remove = (index: number) => { setDirty(true); setItems((current) => current?.filter((_, i) => i !== index) ?? null); };
  const add = () => { setDirty(true); setFilter(""); setItems((current) => [...(current ?? []), { question: "", answer: "" }]); };

  const visible = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return (items ?? []).map((item, index) => ({ item, index })).filter(({ item }) => !q || item.question.toLowerCase().includes(q) || item.answer.toLowerCase().includes(q));
  }, [items, filter]);

  async function save() {
    if (!items) return;
    setBusy(true);
    setError(null);
    try {
      onClose(await saveRobotx(gameId, items));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  }

  return (
    <div role="dialog" aria-modal="true" aria-label={`RobotX de ${title}`} className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-6 backdrop-blur-sm">
      <div className="flex max-h-full w-full max-w-[860px] flex-col overflow-hidden rounded-xl border border-white/10 bg-[#0e0e11] shadow-2xl">
        <header className="flex items-start justify-between gap-4 border-b border-white/5 px-5 py-3">
          <div>
            <p className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">RobotX</p>
            <h3 className="text-sm font-semibold text-zinc-100">{title} · perguntas e respostas</h3>
            <p className="mt-1 text-xs text-zinc-500">O jogador pergunta no chat com @RobotX; o app responde com a resposta da pergunta mais parecida. Escreva perguntas do jeito que um jogador perguntaria.</p>
          </div>
          <button type="button" onClick={close} disabled={busy} aria-label="Fechar" className="rounded-md p-1.5 text-zinc-500 hover:bg-white/5 hover:text-zinc-200"><IconClose /></button>
        </header>

        <div className="flex items-center gap-3 border-b border-white/5 px-5 py-3">
          <TextInput value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Buscar nas perguntas e respostas…" className="flex-1" />
          <span className="text-xs whitespace-nowrap text-zinc-500">{items?.length ?? 0} itens</span>
          <Button size="sm" onClick={add}><IconPlus /> Nova pergunta</Button>
        </div>

        <div className="min-h-[240px] flex-1 space-y-3 overflow-y-auto px-5 py-4">
          {items === null && !error && <p className="text-sm text-zinc-500">Carregando…</p>}
          {visible.map(({ item, index }) => (
            <div key={index} className="rounded-lg border border-white/10 bg-zinc-950/60 p-3">
              <div className="flex items-start gap-2">
                <span className="mt-2 w-7 flex-shrink-0 text-right font-mono text-[11px] text-zinc-600">{index + 1}</span>
                <div className="min-w-0 flex-1 space-y-2">
                  <TextInput value={item.question} maxLength={MAX_QUESTION} onChange={(e) => update(index, { question: e.target.value })} placeholder="Pergunta (ex.: Quais são os controles?)" />
                  <textarea
                    value={item.answer}
                    maxLength={MAX_ANSWER}
                    rows={3}
                    onChange={(e) => update(index, { answer: e.target.value })}
                    placeholder="Resposta do RobotX"
                    className="w-full resize-y rounded-md border border-white/10 bg-zinc-950 px-3 py-2 text-sm leading-relaxed text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-cyan-400/60 focus:ring-2 focus:ring-cyan-400/15"
                  />
                </div>
                <button type="button" onClick={() => remove(index)} aria-label={`Remover pergunta ${index + 1}`} title="Remover" className="mt-1.5 rounded p-1 text-zinc-500 hover:bg-white/5 hover:text-red-300"><IconClose width={14} height={14} /></button>
              </div>
            </div>
          ))}
          {items !== null && visible.length === 0 && <p className="text-sm text-zinc-500">{filter ? "Nada corresponde à busca." : "Nenhuma pergunta. Use \"Nova pergunta\"."}</p>}
        </div>

        <footer className="flex items-center gap-3 border-t border-white/5 px-5 py-3">
          <p role={error ? "alert" : undefined} className="min-w-0 flex-1 text-xs text-red-300">{error}</p>
          <Button variant="info" onClick={close} disabled={busy}>Cancelar</Button>
          <Button variant="success" onClick={() => void save()} loading={busy} disabled={!items}>Salvar e publicar</Button>
        </footer>
      </div>
    </div>
  );
}
