import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { createGame, saveGame, type CatalogGame } from "../../lib/backend";
import type { GameSuggestion } from "../../lib/ai";
import Button from "../../components/ui/Button";
import { Field, TextInput } from "../../components/ui/Field";
import { IconAlert, IconClose, IconTrash } from "../../components/ui/Icons";
import Switch from "../../components/ui/Switch";
import AiAssist from "./AiAssist";
import DeleteGameDialog from "./DeleteGameDialog";
import CoverField from "./CoverField";
import { RobotxField } from "./RobotxEditor";
import RankedValidationBadge from "./RankedValidationBadge";
import type { RankedValidation } from "../../lib/rankedValidation";
import { MODES, PLAY_ON, formatDateTime, playOnOf, slugFromName } from "./options";

interface GameEditorProps {
  /** Jogo em edição; `null` cria um novo. */
  game: CatalogGame | null;
  /** Catálogo inteiro, para recusar um ID que já existe antes de ir ao backend. */
  games: CatalogGame[];
  /** Como o Client valida o Ranked de cada jogo (só leitura: vem das listas do Client). */
  ranked: { byGame: Map<string, RankedValidation> | null; error: string | null };
  onClose: () => void;
  onSaved: (game: CatalogGame) => void;
  onDeleted: (game: CatalogGame) => void;
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
  platforms: string[];
  modes: string[];
  coverUrl: string;
  /** Ranked no MiSTer: o agente lê o placar da memória lá (hoje só o Earthion, core Mega Drive). */
  rankedMister: boolean;
  /** Tempo do Caravan como digitado: minutos (`5`) ou minutos e segundos (`2:30`). */
  caravan: string;
}

/** Tempo do Caravan quando o jogo ainda não tem um (a caravana clássica, 5 min). */
const CARAVAN_PADRAO_SEGUNDOS = 300;

/** `300` → `5:00`. */
function caravanText(segundos: number): string {
  return `${Math.floor(segundos / 60)}:${String(segundos % 60).padStart(2, "0")}`;
}

/** `5` ou `5:00` → 300; `2:30` → 150; inválido → `null`. */
function caravanSeconds(text: string): number | null {
  const match = /^(\d{1,2})(?::([0-5]\d))?$/.exec(text.trim());
  return match ? Number(match[1]) * 60 + Number(match[2] ?? 0) : null;
}

const ID_PATTERN = /^[A-Za-z0-9_-]{1,128}$/;

function toDraft(game: CatalogGame | null): Draft {
  if (!game) {
    return { id: "", title: "", steamAppId: "0", minutes: "60", published: false, developer: "", publisher: "", genre: "", year: "", platform: "", platforms: [], modes: [], coverUrl: "", rankedMister: false, caravan: caravanText(CARAVAN_PADRAO_SEGUNDOS) };
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
    platforms: playOnOf(game),
    modes: game.modes,
    coverUrl: game.cover_url ?? "",
    rankedMister: game.ranked_mister ?? false,
    caravan: caravanText(game.caravan_segundos ?? CARAVAN_PADRAO_SEGUNDOS),
  };
}

/** Jogo já cadastrado com o mesmo ID (sem diferenciar maiúsculas), ao criar um novo. */
function takenBy(draft: Draft, games: CatalogGame[]): CatalogGame | undefined {
  const id = draft.id.trim().toLowerCase();
  return id ? games.find((game) => game.id.toLowerCase() === id) : undefined;
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
  if (draft.modes.includes("caravan")) {
    const caravan = caravanSeconds(draft.caravan);
    if (caravan === null || caravan < 30 || caravan > 3600) return "Tempo do Caravan: de 0:30 a 60:00 (minutos ou m:ss, ex.: 5 ou 2:30).";
    if (caravan >= minutes * 60) return "O tempo do Caravan deve ser menor que a duração máxima da run.";
  }
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

/**
 * Ranked pela leitura de memória, por plataforma. Emulador: fixo, marcado quando o ShmupX Emulator já
 * tem o mapa de memória do jogo (programação interna, `jogos-pela-memoria.json`). MiSTer: o admin liga
 * quando o agente do MiSTer lê o placar do jogo; desligado, o jogo no MiSTer é só Training.
 */
function RankedMemoryFlags({ id, ranked, draft, onRankedMister }: { id: string; ranked: GameEditorProps["ranked"]; draft: Draft; onRankedMister: (on: boolean) => void }) {
  const roms = ranked.byGame?.get(id)?.roms ?? [];
  const onMister = draft.platforms.includes("mister");
  return (
    <div className="space-y-2">
      <span className="block text-xs font-medium text-zinc-400">Ranked pela leitura de memória</span>
      <Switch
        checked={roms.length > 0}
        onChange={() => undefined}
        disabled
        label="Emulador (ShmupX Emulator)"
        description={roms.length > 0
          ? `Fixo: o ShmupX Emulator já lê o placar da memória deste jogo (ROM ${roms.join(", ")}). Vem da programação do Client.`
          : "O ShmupX Emulator ainda não tem o mapa de memória deste jogo. Liga sozinho quando a leitura for programada no Client."}
      />
      <Switch
        checked={draft.rankedMister && onMister}
        onChange={onRankedMister}
        disabled={!onMister}
        label="MiSTer"
        description={onMister
          ? "Ligue só se o agente do MiSTer lê o placar deste jogo (hoje só o Earthion, no core de Mega Drive). Desligado, no MiSTer o jogo é só Training."
          : "Marque MiSTer nas plataformas para liberar."}
      />
    </div>
  );
}

/** Validação do Ranked no Client: só leitura, vem de `jogos-pela-memoria.json` e `scripts-aceitos.json`. */
function RankedValidationField({ id, ranked }: { id: string; ranked: GameEditorProps["ranked"] }) {
  const validation = ranked.byGame?.get(id);
  const roms = validation?.roms ?? [];
  const detail = ranked.error
    ? `Não deu para ler as listas do Client: ${ranked.error}`
    : roms.length > 0
      ? `O ShmupX Emulator lê o placar da memória do jogo (ROM ${roms.join(", ")}): o Ranked não precisa de script de vídeo.`
      : validation?.script
        ? `O Client valida as runs pelo vídeo, com o script ${validation.script} aprovado.`
        : "Sem mapa de memória nem script aprovado: no Client o jogo fica como Coming soon (só Training Mode).";
  return (
    <div>
      <span className="mb-1.5 block text-xs font-medium text-zinc-400">Validação do Ranked</span>
      {ranked.byGame ? <RankedValidationBadge validation={validation} /> : <span className="text-xs text-zinc-600">—</span>}
      <span className="mt-1.5 block text-xs leading-relaxed text-zinc-500">
        {detail} Definido no Client (jogos-pela-memoria.json e scripts-aceitos.json).
      </span>
    </div>
  );
}

/** Painel lateral de criação/edição de um jogo do catálogo. */
export default function GameEditor({ game, games, ranked, onClose, onSaved, onDeleted }: GameEditorProps) {
  const isNew = game === null;
  const [draft, setDraft] = useState<Draft>(() => toDraft(game));
  /** Num jogo novo, o ID acompanha o título até o admin editá-lo à mão. */
  const [idTouched, setIdTouched] = useState(!isNew);
  /** `steam_app_id` que a IA sugeriu sem a Steam confirmar; o aviso some ao editar o campo. */
  const [unverifiedSteamId, setUnverifiedSteamId] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const duplicate = isNew ? takenBy(draft, games) : undefined;

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      // Com a confirmação de exclusão aberta, o Esc fecha só ela.
      if (event.key === "Escape" && !saving && !confirmDelete) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, saving, confirmDelete]);

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
      // A sugestão da IA marca onde jogar; o que o admin já marcou continua.
      platforms: [...new Set([...current.platforms, ...playOnOf({ platform: s.platform ?? undefined, steam_app_id: s.steam_app_id ?? 0 })])],
      modes: s.modes,
      // Sem app_id na sugestão, mantém o que já estava digitado.
      steamAppId: s.steam_app_id ? String(s.steam_app_id) : current.steamAppId,
    }));
    setUnverifiedSteamId(verified ? null : s.steam_app_id);
  }

  function togglePlatform(value: string) {
    patch({ platforms: draft.platforms.includes(value) ? draft.platforms.filter((p) => p !== value) : [...draft.platforms, value] });
  }

  function toggleMode(mode: string) {
    patch({ modes: draft.modes.includes(mode) ? draft.modes.filter((m) => m !== mode) : [...draft.modes, mode] });
  }

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    const invalid = problem(draft);
    if (invalid) return setError(invalid);
    if (duplicate) return setError(`Já existe um jogo com o ID ${duplicate.id} (${duplicate.title}).`);

    setSaving(true);
    setError(null);
    try {
      const save = isNew ? createGame : saveGame;
      const saved = await save(draft.id.trim(), {
        title: draft.title.trim(),
        steam_app_id: Number(draft.steamAppId),
        max_duration_segundos: Math.round(Number(draft.minutes) * 60),
        published: draft.published,
        developer: draft.developer.trim() || undefined,
        publisher: draft.publisher.trim() || undefined,
        genre: draft.genre.trim() || undefined,
        year: draft.year.trim() ? Number(draft.year) : undefined,
        platform: draft.platform || undefined,
        platforms: PLAY_ON.map((p) => p.value).filter((value) => draft.platforms.includes(value)),
        modes: draft.modes,
        cover_url: draft.coverUrl || undefined,
        ranked_mister: draft.rankedMister && draft.platforms.includes("mister"),
        caravan_segundos: draft.modes.includes("caravan") ? caravanSeconds(draft.caravan) ?? undefined : undefined,
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
          {!isNew && (
            <p className="mt-1 text-xs text-zinc-500 tabular-nums">
              Criado em {formatDateTime(game.created_at)} · Atualizado em {formatDateTime(game.updated_at)}
            </p>
          )}
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
              <Field
                label="ID"
                hint={
                  duplicate ? (
                    <span className="flex gap-1.5 text-red-300">
                      <IconAlert className="mt-px flex-shrink-0" />
                      Já existe um jogo com este ID: {duplicate.title}. Escolha outro.
                    </span>
                  ) : isNew ? (
                    "Nome curto, igual ao do validador. Não pode ser alterado depois."
                  ) : (
                    "O ID não muda depois de criado."
                  )
                }
              >
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

          <Section title="Card no Client">
            <CoverField gameId={draft.id} title={draft.title.trim()} value={draft.coverUrl} onChange={(coverUrl) => patch({ coverUrl })} />
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
              <div className="col-span-2">
                <p className="mb-1.5 text-xs font-medium text-zinc-400">Plataformas</p>
                <div className="flex flex-wrap gap-2" role="group" aria-label="Plataformas">
                  {PLAY_ON.map((platform) => {
                    const on = draft.platforms.includes(platform.value);
                    return (
                      <button
                        key={platform.value}
                        type="button"
                        aria-pressed={on}
                        onClick={() => togglePlatform(platform.value)}
                        className={`rounded-full px-3 py-1 text-xs ring-1 transition-colors focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none ${
                          on ? "bg-cyan-400/15 text-cyan-200 ring-cyan-400/40" : "text-zinc-400 ring-white/10 hover:text-zinc-200 hover:ring-white/20"
                        }`}
                      >
                        {platform.label}
                      </button>
                    );
                  })}
                </div>
                <p className="mt-1.5 text-xs text-zinc-500">Onde dá para jogar no ShmupX. Marque todas as que valem.</p>
              </div>
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

          <Section title="RobotX">
            <RobotxField gameId={draft.id.trim()} title={draft.title.trim()} isNew={isNew} />
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
                <Field
                  label="Tempo do Caravan"
                  hint={draft.modes.includes("caravan")
                    ? "Minutos ou m:ss (ex.: 5 ou 2:30). Na partida Caravan, o jogo fecha nesse tempo e o placar feito até ali vale."
                    : "Marque o modo Caravan em Detalhes para definir o tempo."}
                >
                  <TextInput
                    inputMode="numeric"
                    placeholder="5:00"
                    disabled={!draft.modes.includes("caravan")}
                    value={draft.caravan}
                    onChange={(e) => patch({ caravan: e.target.value })}
                  />
                </Field>
              </div>
              <RankedValidationField id={draft.id.trim()} ranked={ranked} />
              <RankedMemoryFlags id={draft.id.trim()} ranked={ranked} draft={draft} onRankedMister={(rankedMister) => patch({ rankedMister })} />
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
          {!isNew && (
            <Button
              variant="ghost"
              onClick={() => setConfirmDelete(true)}
              disabled={saving}
              title="Excluir jogo"
              aria-label="Excluir jogo"
              className="text-red-300 hover:bg-red-500/10 hover:text-red-200"
            >
              <IconTrash />
            </Button>
          )}
          <p role={error ? "alert" : undefined} className="min-w-0 flex-1 text-xs text-red-300">{error}</p>
          <Button variant="info" onClick={onClose} disabled={saving}>Cancelar</Button>
          <Button variant="success" type="submit" loading={saving} disabled={Boolean(duplicate)}>{isNew ? "Criar jogo" : "Salvar"}</Button>
        </footer>
      </form>

      {confirmDelete && game && <DeleteGameDialog game={game} onClose={() => setConfirmDelete(false)} onDeleted={() => onDeleted(game)} />}
    </aside>
  );
}
