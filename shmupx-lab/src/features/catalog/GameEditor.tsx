import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { saveGame, type CatalogGame } from "../../lib/backend";
import type { GameSuggestion } from "../../lib/ai";
import Button from "../../components/ui/Button";
import { Field, Select, TextInput } from "../../components/ui/Field";
import { IconAlert, IconClose } from "../../components/ui/Icons";
import Switch from "../../components/ui/Switch";
import AiAssist from "./AiAssist";
import { MODES, PLATFORMS, slugFromName } from "./options";

interface GameEditorProps {
  /** Jogo em edição; `null` cria um novo. */
  game: CatalogGame | null;
  onClose: () => void;
  onSaved: (game: CatalogGame) => void;
}

interface Draft {
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

const ID_PATTERN = /^[A-Za-z0-9_-]{1,128}$/;

function toDraft(game: CatalogGame | null): Draft {
  if (!game) {
    return { id: "", title: "", steamAppId: "0", minutes: "60", published: false, developer: "", publisher: "", genre: "", year: "", platform: "", modes: [] };
  }
  return {
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

/** Valida o rascunho; devolve a mensagem do primeiro problema ou `null`. */
function problem(draft: Draft): string | null {
  const steamAppId = Number(draft.steamAppId);
  const minutes = Number(draft.minutes);
  const year = draft.year.trim() ? Number(draft.year) : null;
  if (!draft.title.trim()) return "Informe o título do jogo.";
  if (!ID_PATTERN.test(draft.id.trim())) return "ID inválido: use letras, números, - e _ (ex.: earthion).";
  if (!Number.isInteger(steamAppId) || steamAppId < 0) return "app_id da Steam deve ser um inteiro (0 = fora da Steam).";
  if (!Number.isFinite(minutes) || minutes < 1 || minutes > 120) return "Duração máxima da run: de 1 a 120 minutos.";
  if (year !== null && (!Number.isInteger(year) || year < 1970 || year > 2100)) return "Ano deve ficar entre 1970 e 2100.";
  return null;
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h3 className="mb-3 text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">{title}</h3>
      {children}
    </section>
  );
}

/** Painel lateral de criação/edição de um jogo do catálogo. */
export default function GameEditor({ game, onClose, onSaved }: GameEditorProps) {
  const isNew = game === null;
  const [draft, setDraft] = useState<Draft>(() => toDraft(game));
  /** Num jogo novo, o ID acompanha o título até o admin editá-lo à mão. */
  const [idTouched, setIdTouched] = useState(!isNew);
  /** `steam_app_id` que a IA sugeriu sem a Steam confirmar; o aviso some ao editar o campo. */
  const [unverifiedSteamId, setUnverifiedSteamId] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !saving) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, saving]);

  const patch = (changes: Partial<Draft>) => setDraft((current) => ({ ...current, ...changes }));

  function setTitle(title: string) {
    patch(idTouched ? { title } : { title, id: slugFromName(title) });
  }

  function applySuggestion(s: GameSuggestion, verified: boolean) {
    setDraft((current) => ({
      ...current,
      title: s.name,
      id: idTouched ? current.id : slugFromName(s.name),
      developer: s.developer ?? "",
      publisher: s.publisher ?? "",
      genre: s.genre ?? "",
      year: s.year ? String(s.year) : "",
      platform: s.platform ?? "",
      modes: s.modes,
      // Sem app_id na sugestão, mantém o que já estava digitado.
      steamAppId: s.steam_app_id ? String(s.steam_app_id) : current.steamAppId,
    }));
    setUnverifiedSteamId(verified ? null : s.steam_app_id);
  }

  function toggleMode(mode: string) {
    patch({ modes: draft.modes.includes(mode) ? draft.modes.filter((m) => m !== mode) : [...draft.modes, mode] });
  }

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    const invalid = problem(draft);
    if (invalid) return setError(invalid);

    setSaving(true);
    setError(null);
    try {
      const saved = await saveGame(draft.id.trim(), {
        title: draft.title.trim(),
        steam_app_id: Number(draft.steamAppId),
        max_duration_segundos: Math.round(Number(draft.minutes) * 60),
        published: draft.published,
        developer: draft.developer.trim() || undefined,
        publisher: draft.publisher.trim() || undefined,
        genre: draft.genre.trim() || undefined,
        year: draft.year.trim() ? Number(draft.year) : undefined,
        platform: draft.platform || undefined,
        modes: draft.modes,
      });
      onSaved(saved);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  const steamUnverified = unverifiedSteamId !== null && Number(draft.steamAppId) === unverifiedSteamId;

  return (
    <aside
      aria-label={isNew ? "Novo jogo" : `Editar ${game.title}`}
      className="absolute inset-y-0 right-0 z-20 flex w-full max-w-[480px] flex-col border-l border-white/10 bg-[#0e0e11] shadow-2xl shadow-black/60 xl:static xl:z-auto xl:shadow-none"
    >
      <header className="flex items-start justify-between gap-3 border-b border-white/5 px-6 py-4">
        <div className="min-w-0">
          <p className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">{isNew ? "Novo jogo" : "Editar jogo"}</p>
          <h2 className="mt-0.5 truncate text-base font-semibold text-zinc-100">{draft.title.trim() || "Sem título"}</h2>
          {draft.id && <p className="truncate font-mono text-xs text-zinc-500">{draft.id}</p>}
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label="Fechar"
          className="rounded-md p-1.5 text-zinc-500 transition-colors hover:bg-white/5 hover:text-zinc-200 focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none"
        >
          <IconClose />
        </button>
      </header>

      <form onSubmit={onSubmit} className="flex min-h-0 flex-1 flex-col">
        <div className="flex-1 space-y-7 overflow-y-auto px-6 py-5">
          <Section title="Identificação">
            <div className="space-y-4">
              <Field label="Título">
                <TextInput value={draft.title} maxLength={80} autoFocus={isNew} onChange={(e) => setTitle(e.target.value)} placeholder="ex.: Earthion" />
              </Field>
              <Field label="ID" hint={isNew ? "Nome curto, igual ao do validador. Não pode ser alterado depois." : "O ID não muda depois de criado."}>
                <TextInput
                  value={draft.id}
                  disabled={!isNew}
                  className="font-mono"
                  onChange={(e) => {
                    setIdTouched(true);
                    patch({ id: e.target.value.toLowerCase() });
                  }}
                  placeholder="ex.: earthion"
                />
              </Field>
            </div>
          </Section>

          <AiAssist title={draft.title} onApply={applySuggestion} />

          <Section title="Detalhes">
            <div className="grid grid-cols-2 gap-4">
              <Field label="Desenvolvedora" className="col-span-2">
                <TextInput value={draft.developer} maxLength={80} onChange={(e) => patch({ developer: e.target.value })} />
              </Field>
              <Field label="Publicadora" className="col-span-2">
                <TextInput value={draft.publisher} maxLength={80} onChange={(e) => patch({ publisher: e.target.value })} />
              </Field>
              <Field label="Plataforma">
                <Select value={draft.platform} onChange={(e) => patch({ platform: e.target.value })}>
                  <option value="">Não informada</option>
                  {PLATFORMS.map((platform) => (
                    <option key={platform.value} value={platform.value}>{platform.label}</option>
                  ))}
                </Select>
              </Field>
              <Field label="Ano">
                <TextInput inputMode="numeric" maxLength={4} value={draft.year} onChange={(e) => patch({ year: e.target.value })} />
              </Field>
              <Field label="Gênero" className="col-span-2">
                <TextInput value={draft.genre} maxLength={80} onChange={(e) => patch({ genre: e.target.value })} placeholder="ex.: Shmup vertical" />
              </Field>
              <div className="col-span-2">
                <p className="mb-1.5 text-xs font-medium text-zinc-400">Modos de jogo</p>
                <div className="flex flex-wrap gap-2" role="group" aria-label="Modos de jogo">
                  {MODES.map((mode) => {
                    const on = draft.modes.includes(mode.value);
                    return (
                      <button
                        key={mode.value}
                        type="button"
                        aria-pressed={on}
                        onClick={() => toggleMode(mode.value)}
                        className={`rounded-full px-3 py-1 text-xs ring-1 transition-colors focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none ${
                          on ? "bg-cyan-400/15 text-cyan-200 ring-cyan-400/40" : "text-zinc-400 ring-white/10 hover:text-zinc-200 hover:ring-white/20"
                        }`}
                      >
                        {mode.label}
                      </button>
                    );
                  })}
                </div>
              </div>
            </div>
          </Section>

          <Section title="Competição">
            <div className="space-y-4">
              <div className="grid grid-cols-2 gap-4">
                <Field
                  label="app_id da Steam"
                  hint={
                    steamUnverified ? (
                      <span className="flex gap-1.5 text-amber-300">
                        <IconAlert className="mt-px flex-shrink-0" />
                        Sugerido pela IA, não conferido. Veja store.steampowered.com/app/{draft.steamAppId}
                      </span>
                    ) : (
                      "0 = jogo fora da Steam."
                    )
                  }
                >
                  <TextInput
                    inputMode="numeric"
                    value={draft.steamAppId}
                    onChange={(e) => {
                      patch({ steamAppId: e.target.value });
                      setUnverifiedSteamId(null);
                    }}
                  />
                </Field>
                <Field label="Duração máx. da run" hint="Em minutos (1 a 120).">
                  <TextInput inputMode="numeric" value={draft.minutes} onChange={(e) => patch({ minutes: e.target.value })} />
                </Field>
              </div>
              <Switch
                checked={draft.published}
                onChange={(published) => patch({ published })}
                label="Publicado"
                description="Visível no Client e liberado para runs. Desligado, o jogo fica como rascunho."
              />
            </div>
          </Section>
        </div>

        <footer className="flex items-center gap-3 border-t border-white/5 bg-[#0e0e11] px-6 py-3">
          <p role={error ? "alert" : undefined} className="min-w-0 flex-1 text-xs text-red-300">{error}</p>
          <Button variant="info" onClick={onClose} disabled={saving}>Cancelar</Button>
          <Button variant="success" type="submit" loading={saving}>{isNew ? "Criar jogo" : "Salvar"}</Button>
        </footer>
      </form>
    </aside>
  );
}
