import { useState, type FormEvent } from "react";
import { saveGame, type CatalogGame } from "../../lib/backend";
import { suggestGame, type GameSuggestion } from "../../lib/ai";
import { MODES, PLATFORMS, platformLabel, slugFromName } from "./options";

interface CatalogViewProps {
  games: CatalogGame[];
  loading: boolean;
  error: string | null;
  onReload: () => void;
  onSaved: (game: CatalogGame) => void;
}

interface Draft {
  /** `true` enquanto o jogo ainda não existe: só então o ID pode ser digitado. */
  isNew: boolean;
  id: string;
  title: string;
  steamAppId: string;
  minutes: string;
  published: boolean;
  developer: string;
  publisher: string;
  genre: string;
  year: string;
  platform: string;
  modes: string[];
}

type AiState =
  | { status: "idle" }
  | { status: "loading"; web: boolean }
  | { status: "done"; web: boolean; fromCache: boolean; suggestions: GameSuggestion[]; message: string }
  | { status: "error"; message: string };

const EMPTY: Draft = {
  isNew: true, id: "", title: "", steamAppId: "0", minutes: "60", published: false,
  developer: "", publisher: "", genre: "", year: "", platform: "", modes: [],
};
const ID_PATTERN = /^[A-Za-z0-9_-]{1,128}$/;
const FIELD = "w-full rounded-md border border-white/10 bg-zinc-900 px-3 py-2 text-sm text-zinc-100 outline-none focus:border-cyan-400/60 disabled:opacity-50";
const BUTTON = "rounded-md border px-3 py-2 text-sm transition-colors disabled:opacity-50";
const SECONDARY = `${BUTTON} border-white/10 text-zinc-300 hover:bg-zinc-900`;
const PRIMARY = `${BUTTON} border-cyan-400/50 bg-cyan-400/10 text-cyan-200 hover:bg-cyan-400/20`;

function toDraft(game: CatalogGame): Draft {
  return {
    isNew: false,
    id: game.id,
    title: game.title,
    steamAppId: String(game.steam_app_id),
    minutes: String(Math.round(game.max_duration_segundos / 60)),
    published: game.published,
    developer: game.developer ?? "",
    publisher: game.publisher ?? "",
    genre: game.genre ?? "",
    year: game.year ? String(game.year) : "",
    platform: game.platform ?? "",
    modes: game.modes,
  };
}

function suggestionLabel(s: GameSuggestion): string {
  const details = [s.year, platformLabel(s.platform ?? undefined)].filter(Boolean).join(", ");
  return `${s.name}${details ? ` (${details})` : ""}${s.developer ? ` — ${s.developer}` : ""}`;
}

export default function CatalogView({ games, loading, error, onReload, onSaved }: CatalogViewProps) {
  const [draft, setDraft] = useState<Draft>(EMPTY);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ kind: "ok" | "error"; text: string } | null>(null);
  const [ai, setAi] = useState<AiState>({ status: "idle" });
  /** `steam_app_id` que a IA propôs e o admin ainda não conferiu; some ao editar o campo. */
  const [aiSteamId, setAiSteamId] = useState<number | null>(null);

  const patch = (changes: Partial<Draft>) => setDraft((current) => ({ ...current, ...changes }));
  const aiBusy = ai.status === "loading";
  const steamUnverified = aiSteamId !== null && Number(draft.steamAppId) === aiSteamId;

  function applySuggestion(s: GameSuggestion) {
    setDraft((current) => ({
      ...current,
      id: current.isNew && !current.id.trim() ? slugFromName(s.name) : current.id,
      title: s.name,
      developer: s.developer ?? "",
      publisher: s.publisher ?? "",
      genre: s.genre ?? "",
      year: s.year ? String(s.year) : "",
      platform: s.platform ?? "",
      modes: s.modes,
      // Sem palpite da IA, mantém o que já estava digitado.
      steamAppId: s.steam_app_id ? String(s.steam_app_id) : current.steamAppId,
    }));
    setAiSteamId(s.steam_app_id);
  }

  async function askAi(web: boolean, ignoreCache = false) {
    if (!draft.title.trim()) return;
    setAi({ status: "loading", web });
    try {
      const result = await suggestGame(draft.title, web, ignoreCache);
      const count = result.sugestoes.length;
      const origin = result.do_cache ? " (do cache, sem custo)" : "";
      const text =
        count === 0 ? `Nenhum jogo encontrado com esse nome${origin}.`
        : count === 1 ? `1 jogo encontrado${origin}: campos preenchidos. Revise antes de salvar.`
        : `${count} jogos encontrados${origin}. Escolha o correto abaixo.`;
      setAi({ status: "done", web, fromCache: result.do_cache, suggestions: result.sugestoes, message: text });
      if (count === 1) applySuggestion(result.sugestoes[0]);
    } catch (e) {
      setAi({ status: "error", message: e instanceof Error ? e.message : String(e) });
    }
  }

  async function save(next: Draft) {
    const id = next.id.trim();
    const steamAppId = Number(next.steamAppId);
    const minutes = Number(next.minutes);
    const year = next.year.trim() ? Number(next.year) : undefined;
    if (!ID_PATTERN.test(id)) return setMessage({ kind: "error", text: "ID inválido: use letras, números, - e _ (ex.: earthion)." });
    if (!next.title.trim()) return setMessage({ kind: "error", text: "Informe o título do jogo." });
    if (!Number.isInteger(steamAppId) || steamAppId < 0) return setMessage({ kind: "error", text: "app_id da Steam deve ser um inteiro (0 = sem Steam)." });
    if (!Number.isFinite(minutes) || minutes < 1 || minutes > 120) return setMessage({ kind: "error", text: "Duração máxima da run: de 1 a 120 minutos." });
    if (year !== undefined && (!Number.isInteger(year) || year < 1970 || year > 2100)) return setMessage({ kind: "error", text: "Ano deve ficar entre 1970 e 2100." });

    setSaving(true);
    setMessage(null);
    try {
      const saved = await saveGame(id, {
        title: next.title.trim(),
        steam_app_id: steamAppId,
        max_duration_segundos: Math.round(minutes * 60),
        published: next.published,
        developer: next.developer.trim() || undefined,
        publisher: next.publisher.trim() || undefined,
        genre: next.genre.trim() || undefined,
        year,
        platform: next.platform || undefined,
        modes: next.modes,
      });
      onSaved(saved);
      setDraft(EMPTY);
      setAi({ status: "idle" });
      setAiSteamId(null);
      setMessage({ kind: "ok", text: `${saved.title}: ${saved.published ? "publicado no catálogo" : "salvo como rascunho"}.` });
    } catch (e) {
      setMessage({ kind: "error", text: e instanceof Error ? e.message : String(e) });
    } finally {
      setSaving(false);
    }
  }

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    void save(draft);
  }

  function startEditing(game: CatalogGame) {
    setMessage(null);
    setAi({ status: "idle" });
    setAiSteamId(null);
    setDraft(toDraft(game));
  }

  function toggleMode(mode: string, on: boolean) {
    patch({ modes: on ? [...draft.modes, mode] : draft.modes.filter((m) => m !== mode) });
  }

  return (
    <section className="flex flex-1 flex-col gap-6 overflow-y-auto p-8">
      <header className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold text-zinc-100">Catálogo de jogos</h1>
          <p className="mt-1 text-xs text-zinc-500">
            Um jogo = uma sala = um RobotX = um validador, todos pelo mesmo ID. Publicar faz o jogo aparecer no Client e liberar runs.
          </p>
        </div>
        <button type="button" onClick={onReload} disabled={loading} className={SECONDARY}>
          {loading ? "Carregando…" : "Recarregar"}
        </button>
      </header>

      {error && <p role="alert" className="rounded-md border border-red-500/40 bg-red-500/10 px-4 py-3 text-sm text-red-200">{error}</p>}
      {message && (
        <p role="status" className={`rounded-md border px-4 py-3 text-sm ${message.kind === "ok" ? "border-emerald-500/40 bg-emerald-500/10 text-emerald-200" : "border-red-500/40 bg-red-500/10 text-red-200"}`}>
          {message.text}
        </p>
      )}

      <div className="overflow-hidden rounded-lg border border-white/5">
        <table className="w-full text-left text-sm">
          <thead className="bg-zinc-900 text-xs uppercase tracking-wider text-zinc-500">
            <tr><th className="px-4 py-3">Jogo</th><th className="px-4 py-3">ID</th><th className="px-4 py-3">Steam</th><th className="px-4 py-3">Run máx.</th><th className="px-4 py-3">Estado</th><th className="px-4 py-3" /></tr>
          </thead>
          <tbody>
            {games.length === 0 && (
              <tr><td colSpan={6} className="px-4 py-6 text-center text-zinc-500">{loading ? "Carregando…" : "Nenhum jogo cadastrado ainda."}</td></tr>
            )}
            {games.map((game) => {
              const subtitle = [game.developer, game.year, platformLabel(game.platform)].filter(Boolean).join(" · ");
              return (
                <tr key={game.id} className="border-t border-white/5">
                  <td className="px-4 py-3 text-zinc-100">
                    {game.title}
                    {subtitle && <span className="block text-xs text-zinc-500">{subtitle}</span>}
                  </td>
                  <td className="px-4 py-3 font-mono text-xs text-zinc-400">{game.id}</td>
                  <td className="px-4 py-3 text-zinc-400">{game.steam_app_id || "—"}</td>
                  <td className="px-4 py-3 text-zinc-400">{Math.round(game.max_duration_segundos / 60)} min</td>
                  <td className="px-4 py-3">
                    <span className={`rounded-full px-2 py-0.5 text-xs ${game.published ? "bg-emerald-500/15 text-emerald-300" : "bg-zinc-700/40 text-zinc-400"}`}>
                      {game.published ? "Publicado" : "Rascunho"}
                    </span>
                  </td>
                  <td className="flex justify-end gap-2 px-4 py-2">
                    <button type="button" disabled={saving} onClick={() => startEditing(game)} className={SECONDARY}>Editar</button>
                    <button type="button" disabled={saving} onClick={() => void save({ ...toDraft(game), published: !game.published })} className={game.published ? SECONDARY : PRIMARY}>
                      {game.published ? "Despublicar" : "Publicar"}
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      <form onSubmit={onSubmit} className="grid max-w-3xl grid-cols-2 gap-4 rounded-lg border border-white/5 bg-zinc-900/40 p-6">
        <h2 className="col-span-2 text-sm font-semibold text-zinc-200">{draft.isNew ? "Novo jogo" : `Editando ${draft.id}`}</h2>

        <label className="text-xs text-zinc-400">Título
          <input className={`${FIELD} mt-1`} value={draft.title} maxLength={80} onChange={(e) => patch({ title: e.target.value })} placeholder="Earthion" />
        </label>
        <label className="text-xs text-zinc-400">ID (nome curto, igual ao do validador)
          <input className={`${FIELD} mt-1 font-mono`} value={draft.id} disabled={!draft.isNew} onChange={(e) => patch({ id: e.target.value.toLowerCase() })} placeholder="earthion" />
        </label>

        <div className="col-span-2 flex flex-wrap items-center gap-3">
          <button type="button" disabled={aiBusy || !draft.title.trim()} onClick={() => void askAi(false)} className={PRIMARY}>
            {ai.status === "loading" && !ai.web ? "Consultando a IA…" : "Preencher com IA"}
          </button>
          <button type="button" disabled={aiBusy || !draft.title.trim()} onClick={() => void askAi(true)} className={SECONDARY} title="Pesquisa na internet; mais lento e mais caro. Use para títulos obscuros.">
            {ai.status === "loading" && ai.web ? "Pesquisando na internet…" : "Buscar na internet"}
          </button>
          {ai.status === "done" && ai.fromCache && (
            <button type="button" disabled={aiBusy} onClick={() => void askAi(ai.web, true)} className="text-xs text-zinc-500 underline hover:text-zinc-300">
              Consultar de novo (ignora o cache)
            </button>
          )}
          <p className="w-full text-xs text-zinc-500">Digite o título e a IA propõe os campos abaixo. Nada é salvo antes de você revisar e clicar em Salvar.</p>
        </div>
        {ai.status === "error" && <p role="alert" className="col-span-2 rounded-md border border-red-500/40 bg-red-500/10 px-3 py-2 text-sm text-red-200">{ai.message}</p>}
        {ai.status === "done" && (
          <div className="col-span-2 space-y-2">
            <p role="status" className="text-sm text-zinc-300">{ai.message}</p>
            {ai.suggestions.length > 1 && (
              <ul className="space-y-1">
                {ai.suggestions.map((s, index) => (
                  <li key={`${s.name}-${index}`}>
                    <button type="button" onClick={() => applySuggestion(s)} className="w-full rounded-md border border-white/10 px-3 py-2 text-left text-sm text-zinc-200 hover:border-cyan-400/50 hover:bg-cyan-400/10">
                      {suggestionLabel(s)}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}

        <label className="text-xs text-zinc-400">Desenvolvedora
          <input className={`${FIELD} mt-1`} value={draft.developer} maxLength={80} onChange={(e) => patch({ developer: e.target.value })} />
        </label>
        <label className="text-xs text-zinc-400">Publicadora
          <input className={`${FIELD} mt-1`} value={draft.publisher} maxLength={80} onChange={(e) => patch({ publisher: e.target.value })} />
        </label>
        <label className="text-xs text-zinc-400">Gênero
          <input className={`${FIELD} mt-1`} value={draft.genre} maxLength={80} onChange={(e) => patch({ genre: e.target.value })} placeholder="Shmup vertical" />
        </label>
        <label className="text-xs text-zinc-400">Ano
          <input className={`${FIELD} mt-1`} inputMode="numeric" value={draft.year} maxLength={4} onChange={(e) => patch({ year: e.target.value })} placeholder="2021" />
        </label>
        <label className="text-xs text-zinc-400">Plataforma
          <select className={`${FIELD} mt-1`} value={draft.platform} onChange={(e) => patch({ platform: e.target.value })}>
            <option value="">—</option>
            {PLATFORMS.map((platform) => <option key={platform.value} value={platform.value}>{platform.label}</option>)}
          </select>
        </label>
        <fieldset className="text-xs text-zinc-400">
          <legend>Modos de jogo</legend>
          <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1">
            {MODES.map((mode) => (
              <label key={mode.value} className="flex items-center gap-1.5 text-sm text-zinc-300">
                <input type="checkbox" checked={draft.modes.includes(mode.value)} onChange={(e) => toggleMode(mode.value, e.target.checked)} />
                {mode.label}
              </label>
            ))}
          </div>
        </fieldset>

        <label className="text-xs text-zinc-400">app_id da Steam (0 = sem Steam)
          <input className={`${FIELD} mt-1`} inputMode="numeric" value={draft.steamAppId} onChange={(e) => { patch({ steamAppId: e.target.value }); setAiSteamId(null); }} />
          {steamUnverified && (
            <span role="note" className="mt-1 block text-amber-300">
              Sugerido pela IA: confira em store.steampowered.com/app/{draft.steamAppId} antes de salvar.
            </span>
          )}
        </label>
        <label className="text-xs text-zinc-400">Duração máxima da run (minutos)
          <input className={`${FIELD} mt-1`} inputMode="numeric" value={draft.minutes} onChange={(e) => patch({ minutes: e.target.value })} />
        </label>
        <label className="col-span-2 flex items-center gap-2 text-sm text-zinc-300">
          <input type="checkbox" checked={draft.published} onChange={(e) => patch({ published: e.target.checked })} />
          Publicado (visível no Client e liberado para runs)
        </label>
        <div className="col-span-2 flex gap-3">
          <button type="submit" disabled={saving} className={PRIMARY}>{saving ? "Salvando…" : "Salvar"}</button>
          {!draft.isNew && <button type="button" disabled={saving} onClick={() => { setDraft(EMPTY); setMessage(null); setAi({ status: "idle" }); setAiSteamId(null); }} className={SECONDARY}>Cancelar edição</button>}
        </div>
      </form>
    </section>
  );
}
