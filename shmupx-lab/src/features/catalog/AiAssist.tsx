import { useState } from "react";
import { suggestGame, type GameSuggestion } from "../../lib/ai";
import Badge from "../../components/ui/Badge";
import Button from "../../components/ui/Button";
import { IconAlert, IconCheck, IconGlobe, IconSparkles } from "../../components/ui/Icons";
import { platformLabel } from "./options";

interface AiAssistProps {
  /** Título digitado no formulário: é o que a IA pesquisa. */
  title: string;
  /** Aplica a sugestão escolhida no formulário. `verified` = `steam_app_id` conferido pela Steam. */
  onApply: (suggestion: GameSuggestion, verified: boolean) => void;
}

type State =
  | { status: "idle" }
  | { status: "loading"; web: boolean }
  | { status: "error"; message: string }
  | { status: "done"; web: boolean; fromCache: boolean; verified: boolean; suggestions: GameSuggestion[]; warnings: string[] };

function meta(s: GameSuggestion): string {
  return [s.year, platformLabel(s.platform ?? undefined), s.developer].filter(Boolean).join(" · ");
}

/** Preenchimento automático do formulário a partir do título (IA + Steam). Nada é salvo aqui. */
export default function AiAssist({ title, onApply }: AiAssistProps) {
  const [state, setState] = useState<State>({ status: "idle" });
  const [useSteam, setUseSteam] = useState(true);
  const [selected, setSelected] = useState<number | null>(null);

  const busy = state.status === "loading";
  const canQuery = title.trim().length > 0 && !busy;

  function choose(index: number, suggestions: GameSuggestion[], verified: boolean) {
    setSelected(index);
    onApply(suggestions[index], verified);
  }

  async function ask(web: boolean, ignoreCache = false) {
    if (!title.trim()) return;
    setState({ status: "loading", web });
    setSelected(null);
    try {
      const result = await suggestGame(title, { web, steam: useSteam, ignoreCache });
      const verified = useSteam && result.steam_consultada;
      setState({ status: "done", web, fromCache: result.do_cache, verified, suggestions: result.sugestoes, warnings: result.avisos });
      // Uma única sugestão já entra no formulário; com várias, o admin escolhe.
      if (result.sugestoes.length === 1) choose(0, result.sugestoes, verified);
    } catch (e) {
      setState({ status: "error", message: e instanceof Error ? e.message : String(e) });
    }
  }

  return (
    <section aria-label="Preenchimento automático" className="rounded-lg border border-cyan-400/15 bg-cyan-400/[0.03] p-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="flex items-center gap-1.5 text-sm font-medium text-zinc-100">
            <IconSparkles className="text-cyan-300" />
            Preenchimento automático
          </p>
          <p className="mt-0.5 text-xs text-zinc-500">Sugere os campos a partir do título. Nada é salvo antes de você revisar.</p>
        </div>
        <label className="flex flex-shrink-0 items-center gap-1.5 text-xs text-zinc-400" title="Busca o jogo na loja da Steam (gratuito) e usa os dados dela: app_id, desenvolvedora, publicadora e ano.">
          <input type="checkbox" className="accent-cyan-400" checked={useSteam} onChange={(e) => setUseSteam(e.target.checked)} />
          Consultar a Steam
        </label>
      </div>

      <div className="mt-3 flex flex-wrap gap-2">
        <Button variant="primary" size="sm" disabled={!canQuery} loading={state.status === "loading" && !state.web} onClick={() => void ask(false)}>
          {!(state.status === "loading" && !state.web) && <IconSparkles />}
          Preencher com IA
        </Button>
        <Button size="sm" disabled={!canQuery} loading={state.status === "loading" && state.web} onClick={() => void ask(true)} title="Pesquisa na internet: mais lento e mais caro. Para títulos obscuros.">
          {!(state.status === "loading" && state.web) && <IconGlobe />}
          Buscar na internet
        </Button>
      </div>
      {!title.trim() && <p className="mt-2 text-xs text-zinc-500">Digite o título para habilitar.</p>}

      {state.status === "error" && (
        <p role="alert" className="mt-3 flex gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-3 py-2 text-xs text-red-200">
          <IconAlert className="mt-px flex-shrink-0" />
          {state.message}
        </p>
      )}

      {state.status === "done" && (
        <div className="mt-4">
          <div className="flex items-center justify-between gap-2">
            <p role="status" className="text-xs text-zinc-400">
              {state.suggestions.length === 0
                ? "Nenhum jogo encontrado com esse título."
                : state.suggestions.length === 1
                  ? "1 sugestão aplicada. Revise os campos."
                  : `${state.suggestions.length} sugestões. Escolha a versão certa.`}
            </p>
            <div className="flex items-center gap-2">
              {state.fromCache && <Badge>do cache · sem custo</Badge>}
              {state.fromCache && (
                <button type="button" disabled={busy} onClick={() => void ask(state.web, true)} className="text-xs text-zinc-500 underline-offset-2 hover:text-zinc-300 hover:underline">
                  Consultar de novo
                </button>
              )}
            </div>
          </div>

          {state.warnings.map((warning) => (
            <p key={warning} role="note" className="mt-2 flex gap-2 text-xs text-amber-300">
              <IconAlert className="mt-px flex-shrink-0" />
              {warning}
            </p>
          ))}

          {state.suggestions.length > 0 && (
            <ul role="listbox" aria-label="Sugestões" className="mt-2 max-h-64 divide-y divide-white/5 overflow-y-auto rounded-md border border-white/10 bg-zinc-950">
              {state.suggestions.map((s, index) => {
                const isSelected = selected === index;
                return (
                  <li key={`${s.name}-${index}`} role="option" aria-selected={isSelected}>
                    <button
                      type="button"
                      onClick={() => choose(index, state.suggestions, state.verified)}
                      className={`flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors focus-visible:bg-white/[0.06] focus-visible:outline-none ${isSelected ? "bg-cyan-400/10" : "hover:bg-white/[0.03]"}`}
                    >
                      <span aria-hidden className={`grid h-4 w-4 flex-shrink-0 place-items-center rounded-full ring-1 ${isSelected ? "bg-cyan-400 text-zinc-950 ring-cyan-400" : "ring-white/20"}`}>
                        {isSelected && <IconCheck width={10} height={10} strokeWidth={3} />}
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-sm text-zinc-100">{s.name}</span>
                        {meta(s) && <span className="block truncate text-xs text-zinc-500">{meta(s)}</span>}
                      </span>
                      {s.steam_app_id && (
                        <Badge tone={state.verified ? "info" : "warning"}>
                          {state.verified ? `Steam ${s.steam_app_id}` : "app_id não conferido"}
                        </Badge>
                      )}
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      )}
    </section>
  );
}
