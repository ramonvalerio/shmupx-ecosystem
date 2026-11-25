import { useEffect, useMemo, useState, type ReactNode } from "react";
import Badge from "../../components/ui/Badge";
import Button from "../../components/ui/Button";
import { Field, TextInput } from "../../components/ui/Field";
import { IconAlert, IconCheck, IconClose, IconCoins, IconRefresh, IconSearch, IconUsers } from "../../components/ui/Icons";
import Switch from "../../components/ui/Switch";
import CopyButton from "../../components/ui/CopyButton";
import { moderateUser, type AdminUser, type UserModeration, type UserStatus } from "../../lib/backend";
import { isModerated, matchesUser, STATUS_DOT, STATUS_LABEL, timeAgo, userName } from "./options";

type Filter = "all" | "connected" | "away" | "offline" | "moderated";

const FILTERS: { id: Filter; label: string; test: (u: AdminUser) => boolean }[] = [
  { id: "all", label: "Todos", test: () => true },
  { id: "connected", label: "Online", test: (u) => u.status === "online" || u.status === "playing" || u.status === "live" },
  { id: "away", label: "Ausentes", test: (u) => u.status === "away" },
  { id: "offline", label: "Offline", test: (u) => u.status === "offline" },
  { id: "moderated", label: "Moderados", test: isModerated },
];

interface UsersViewProps {
  users: AdminUser[];
  loading: boolean;
  error: string | null;
  updatedAt: Date | null;
  onReload: () => void;
  onModerated: (uid: string, moderation: UserModeration) => void;
  /** Abre o Coins Manager com o jogador escolhido. */
  onSendCoins: (uid: string) => void;
}

/** Users Manager: todos os jogadores, quem está online/ausente/offline e a moderação de cada um. */
export default function UsersView({ users, loading, error, updatedAt, onReload, onModerated, onSendCoins }: UsersViewProps) {
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [selectedUid, setSelectedUid] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 30_000);
    return () => window.clearInterval(timer);
  }, []);
  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 5000);
    return () => window.clearTimeout(timer);
  }, [notice]);

  const counts = useMemo(() => {
    const by = (status: UserStatus[]) => users.filter((u) => status.includes(u.status)).length;
    return { connected: by(["online", "playing", "live"]), playing: by(["playing", "live"]), away: by(["away"]), offline: by(["offline"]), moderated: users.filter(isModerated).length };
  }, [users]);
  const visible = useMemo(() => {
    const test = FILTERS.find((f) => f.id === filter)!.test;
    return users.filter((u) => test(u) && matchesUser(u, search));
  }, [users, filter, search]);
  const selected = selectedUid ? users.find((u) => u.uid === selectedUid) ?? null : null;

  return (
    <div className="relative flex min-w-0 flex-1 overflow-hidden">
      <section className="flex min-w-0 flex-1 flex-col">
        <header className="flex flex-wrap items-end justify-between gap-4 px-8 pt-7 pb-5">
          <div>
            <h1 className="text-xl font-semibold text-zinc-100">Users Manager</h1>
            <p className="mt-1 max-w-2xl text-sm text-zinc-500">
              Todos os jogadores cadastrados no ShmupX, quem está online, ausente ou offline agora, e a moderação: banir, mutar, desativar o acesso e tirar a visibilidade no chat.
            </p>
          </div>
          <div className="flex items-center gap-3">
            {updatedAt && <span className="text-[11px] text-zinc-600" title="A lista se atualiza sozinha a cada 20 s">atualizado {timeAgo(updatedAt.toISOString(), now)}</span>}
            <Button variant="ghost" onClick={onReload} disabled={loading} title="Recarregar" aria-label="Recarregar">
              <IconRefresh className={loading ? "animate-spin" : ""} />
            </Button>
          </div>
        </header>

        <div className="mx-8 mb-4 grid grid-cols-2 gap-3 lg:grid-cols-5">
          <Stat label="Cadastrados" value={users.length} />
          <Stat label="Online agora" value={counts.connected} dot="bg-emerald-400" hint={counts.playing ? `${counts.playing} jogando` : undefined} />
          <Stat label="Ausentes" value={counts.away} dot="bg-amber-400" />
          <Stat label="Offline" value={counts.offline} dot="bg-zinc-600" />
          <Stat label="Moderados" value={counts.moderated} dot="bg-red-400" />
        </div>

        <div className="mx-8 mb-4 flex flex-wrap items-center gap-3">
          <label className="relative w-72">
            <IconSearch className="pointer-events-none absolute top-2.5 left-3 text-zinc-600" />
            <TextInput value={search} onChange={(e) => setSearch(e.target.value)} placeholder="Buscar por nome, @nick, e-mail ou uid" className="pl-9" aria-label="Buscar jogador" />
          </label>
          <div role="radiogroup" aria-label="Filtrar por situação" className="flex gap-1 rounded-lg border border-white/10 bg-zinc-950 p-1">
            {FILTERS.map((f) => (
              <button
                key={f.id}
                type="button"
                role="radio"
                aria-checked={filter === f.id}
                onClick={() => setFilter(f.id)}
                className={`rounded-md px-3 py-1 text-xs font-medium transition-colors focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none ${filter === f.id ? "bg-cyan-400 text-zinc-950" : "text-zinc-400 hover:bg-white/5 hover:text-zinc-100"}`}
              >
                {f.label}
              </button>
            ))}
          </div>
          <span className="text-xs text-zinc-500">{visible.length} de {users.length}</span>
        </div>

        {error && (
          <p role="alert" className="mx-8 mb-4 flex gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
            <IconAlert className="mt-0.5 flex-shrink-0" />
            {error}
          </p>
        )}

        <div className="mx-8 mb-8 min-h-0 flex-1 overflow-auto rounded-lg border border-white/5 bg-zinc-950/40">
          {visible.length === 0 ? (
            <div className="flex h-full min-h-56 flex-col items-center justify-center gap-2 px-6 py-14 text-center">
              <IconUsers className="h-6 w-6 text-zinc-600" />
              <p className="text-sm font-medium text-zinc-200">{loading && users.length === 0 ? "Carregando jogadores…" : "Nenhum jogador neste filtro"}</p>
            </div>
          ) : (
            <table className="w-full text-left text-sm">
              <thead className="sticky top-0 z-10 bg-zinc-950 text-[11px] tracking-wider text-zinc-500 uppercase">
                <tr>
                  <th className="px-4 py-3 font-semibold">Jogador</th>
                  <th className="px-4 py-3 font-semibold">Situação</th>
                  <th className="px-4 py-3 text-right font-semibold">Fichas</th>
                  <th className="px-4 py-3 font-semibold">Último acesso</th>
                  <th className="px-4 py-3 font-semibold">Moderação</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/5">
                {visible.map((user) => (
                  <tr
                    key={user.uid}
                    onClick={() => setSelectedUid(user.uid)}
                    className={`cursor-pointer transition-colors hover:bg-white/[0.03] ${selectedUid === user.uid ? "bg-white/[0.05]" : ""}`}
                  >
                    <td className="px-4 py-2.5">
                      <div className="flex items-center gap-3">
                        <Avatar user={user} />
                        <div className="min-w-0 leading-tight">
                          <p className="flex items-center gap-1.5 truncate text-zinc-100">
                            {userName(user)}
                            {user.is_admin && <Badge tone="info">admin</Badge>}
                          </p>
                          <p className="truncate text-[11px] text-zinc-500">{[user.handle && `@${user.handle}`, user.email].filter(Boolean).join(" · ") || user.uid}</p>
                        </div>
                      </div>
                    </td>
                    <td className="px-4 py-2.5"><Presence user={user} /></td>
                    <td className="px-4 py-2.5 text-right font-mono text-zinc-300 tabular-nums" title={user.balance == null ? "Ainda sem carteira: ganha as fichas iniciais na primeira run" : undefined}>
                      {user.balance == null ? <span className="text-zinc-600">50*</span> : user.balance.toLocaleString("pt-BR")}
                    </td>
                    <td className="px-4 py-2.5 text-xs text-zinc-400" title={user.last_seen_at ? new Date(user.last_seen_at).toLocaleString("pt-BR") : undefined}>
                      {user.status === "offline" ? timeAgo(user.last_seen_at, now) : <span className="text-emerald-300">agora</span>}
                    </td>
                    <td className="px-4 py-2.5"><ModerationBadges moderation={user.moderation} /></td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </section>

      {selected && (
        <UserPanel
          key={selected.uid}
          user={selected}
          now={now}
          onClose={() => setSelectedUid(null)}
          onSendCoins={() => onSendCoins(selected.uid)}
          onSaved={(moderation, message) => {
            onModerated(selected.uid, moderation);
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

function Stat({ label, value, dot, hint }: { label: string; value: number; dot?: string; hint?: string }) {
  return (
    <div className="rounded-lg border border-white/5 bg-zinc-950/60 px-4 py-3">
      <p className="flex items-center gap-2 text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">
        {dot && <span aria-hidden className={`h-2 w-2 rounded-full ${dot}`} />}
        {label}
      </p>
      <p className="mt-1 text-2xl font-semibold text-zinc-100 tabular-nums">
        {value.toLocaleString("pt-BR")}
        {hint && <span className="ml-2 text-xs font-normal text-zinc-500">{hint}</span>}
      </p>
    </div>
  );
}

function Avatar({ user, size = "h-8 w-8" }: { user: AdminUser; size?: string }) {
  const tone = user.moderation.banned ? "bg-red-500/15 text-red-300 ring-red-400/30" : "bg-zinc-800 text-zinc-300 ring-white/5";
  return (
    <span aria-hidden className={`relative grid ${size} flex-shrink-0 place-items-center rounded-full text-xs font-semibold uppercase ring-1 ${tone}`}>
      {userName(user).replace(/^@/, "").charAt(0)}
      <span className={`absolute -right-0.5 -bottom-0.5 h-2.5 w-2.5 rounded-full ring-2 ring-zinc-950 ${STATUS_DOT[user.status]}`} />
    </span>
  );
}

function Presence({ user }: { user: AdminUser }) {
  return (
    <span className="flex items-center gap-2 text-xs">
      <span aria-hidden className={`h-2 w-2 flex-shrink-0 rounded-full ${STATUS_DOT[user.status]}`} />
      <span className={user.status === "offline" ? "text-zinc-500" : "text-zinc-200"}>{STATUS_LABEL[user.status]}</span>
      {user.activity && user.status !== "offline" && <span className="max-w-48 truncate text-zinc-500" title={user.activity}>· {user.activity}</span>}
    </span>
  );
}

function ModerationBadges({ moderation }: { moderation: UserModeration }) {
  const flags = [
    moderation.banned && <Badge key="b" tone="danger">banido</Badge>,
    moderation.disabled && <Badge key="d" tone="danger">acesso desativado</Badge>,
    moderation.muted && <Badge key="m" tone="warning">mutado</Badge>,
    moderation.chat_hidden && <Badge key="h" tone="warning">fora do chat</Badge>,
  ].filter(Boolean);
  return flags.length ? <span className="flex flex-wrap gap-1">{flags}</span> : <span className="text-xs text-zinc-600">—</span>;
}

/** Painel do jogador: dados, situação e a moderação (aplicada ao salvar). */
function UserPanel({ user, now, onClose, onSendCoins, onSaved }: {
  user: AdminUser;
  now: number;
  onClose: () => void;
  onSendCoins: () => void;
  onSaved: (moderation: UserModeration, message: string) => void;
}) {
  const [draft, setDraft] = useState(() => ({ ...user.moderation }));
  const [note, setNote] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const current = user.moderation;
  const changed = (["banned", "muted", "disabled", "chat_hidden"] as const).filter((k) => draft[k] !== current[k]);
  const locked = user.is_admin;

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => { if (event.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  async function save() {
    setSaving(true);
    setError(null);
    try {
      const input = Object.fromEntries(changed.map((k) => [k, draft[k]]));
      const saved = await moderateUser(user.uid, { ...input, note: note.trim() || undefined });
      setDraft({ ...saved });
      setNote("");
      onSaved(saved, `Moderação de ${userName(user)} salva.`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  const set = (key: "banned" | "muted" | "disabled" | "chat_hidden") => (value: boolean) => setDraft((d) => ({ ...d, [key]: value }));
  const facts: [string, ReactNode][] = [
    ["E-mail", user.email ?? "—"],
    ["Nick", user.handle ? `@${user.handle}` : "—"],
    ["Fichas", user.balance == null ? "50 (ainda sem carteira)" : user.balance.toLocaleString("pt-BR")],
    ["Cadastro", user.created_at ? new Date(user.created_at).toLocaleDateString("pt-BR") : "—"],
    ["Último acesso", user.status === "offline" ? timeAgo(user.last_seen_at, now) : "agora"],
    ["Versão do Client", user.client_version ?? "—"],
  ];

  return (
    <aside aria-label={`Jogador ${userName(user)}`} className="absolute inset-y-0 right-0 z-20 flex w-full max-w-[420px] flex-col border-l border-white/10 bg-[#0e0e11] shadow-2xl shadow-black/60 xl:static xl:z-auto xl:shadow-none">
      <header className="flex items-start justify-between gap-3 border-b border-white/5 px-6 py-4">
        <div className="flex min-w-0 items-center gap-3">
          <Avatar user={user} size="h-10 w-10" />
          <div className="min-w-0">
            <h2 className="flex items-center gap-2 truncate text-base font-semibold text-zinc-100">
              {userName(user)}
              {user.is_admin && <Badge tone="info">admin</Badge>}
            </h2>
            <Presence user={user} />
          </div>
        </div>
        <Button variant="ghost" size="sm" onClick={onClose} aria-label="Fechar"><IconClose /></Button>
      </header>

      <div className="min-h-0 flex-1 space-y-6 overflow-auto px-6 py-5">
        <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-sm">
          {facts.map(([label, value]) => (
            <div key={label} className="contents">
              <dt className="text-xs text-zinc-500">{label}</dt>
              <dd className="truncate text-zinc-200">{value}</dd>
            </div>
          ))}
          <dt className="text-xs text-zinc-500">UID</dt>
          <dd className="flex min-w-0 items-center gap-1"><span className="truncate font-mono text-xs text-zinc-400">{user.uid}</span><CopyButton value={user.uid} label="Copiar UID" /></dd>
        </dl>

        <Button variant="secondary" className="w-full" onClick={onSendCoins}>
          <IconCoins />
          Enviar fichas para este jogador
        </Button>

        <section aria-label="Moderação" className="space-y-2">
          <p className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">Moderação</p>
          {locked && <p className="rounded-md border border-cyan-400/20 bg-cyan-400/5 px-3 py-2 text-xs text-cyan-200">Administrador do Lab: não pode ser moderado.</p>}
          <Switch checked={draft.banned} onChange={set("banned")} disabled={locked} label="Banir" description="Fora do ShmupX: não joga valendo ranking, não fala nem aparece nas salas, some do chat e não recebe fichas de distribuições para todos." />
          <Switch checked={draft.disabled} onChange={set("disabled")} disabled={locked} label="Desativar acesso" description="Suspensão: o Client mostra a conta desativada e o jogador não joga, não fala e não aparece nas salas até reativar." />
          <Switch checked={draft.muted} onChange={set("muted")} disabled={locked} label="Mutar" description="Continua jogando e lendo o chat, mas não consegue enviar mensagens." />
          <Switch checked={draft.chat_hidden} onChange={set("chat_hidden")} disabled={locked} label="Tirar visibilidade do chat" description="As mensagens dele deixam de aparecer para os outros jogadores (ele continua vendo as próprias)." />
          {!locked && (
            <Field label="Motivo (opcional)" hint={current.note ? `Último motivo: ${current.note}` : undefined}>
              <TextInput value={note} maxLength={300} onChange={(e) => setNote(e.target.value)} placeholder="Ex.: spam no chat da sala Darius" />
            </Field>
          )}
          {current.updated_at && (
            <p className="text-[11px] text-zinc-600">Última mudança {timeAgo(current.updated_at, now)}{current.updated_by ? ` por ${current.updated_by}` : ""}.</p>
          )}
        </section>
      </div>

      {!locked && (
        <footer className="flex items-center gap-3 border-t border-white/5 px-6 py-3">
          {error ? (
            <p role="alert" className="min-w-0 flex-1 text-xs text-red-300">{error}</p>
          ) : (
            <p className="min-w-0 flex-1 text-xs text-zinc-500">{changed.length ? `${changed.length} mudança(s) para salvar` : "Sem mudanças"}</p>
          )}
          <Button variant="ghost" size="sm" disabled={!changed.length || saving} onClick={() => setDraft({ ...current })}>Desfazer</Button>
          <Button variant={draft.banned && !current.banned ? "danger" : "success"} size="sm" loading={saving} disabled={!changed.length} onClick={() => void save()}>
            {draft.banned && !current.banned ? "Banir jogador" : "Salvar moderação"}
          </Button>
        </footer>
      )}
    </aside>
  );
}
