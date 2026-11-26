import { useCallback, useEffect, useMemo, useState } from "react";
import Badge from "../../components/ui/Badge";
import Button from "../../components/ui/Button";
import { IconAlert, IconMonitor, IconRefresh } from "../../components/ui/Icons";
import { getClientOverview, type ClientOverview, type DownloadPlatform } from "../../lib/backend";

type Period = "week" | "month" | "quarter" | "semester" | "year";

/** Período (terminando hoje), tamanho em dias e como agrupar as barras. */
const PERIODS: { id: Period; label: string; days: number; bucket: "day" | "week" | "month" }[] = [
  { id: "week", label: "Semanal", days: 7, bucket: "day" },
  { id: "month", label: "Mensal", days: 30, bucket: "day" },
  { id: "quarter", label: "Trimestral", days: 91, bucket: "week" },
  { id: "semester", label: "Semestral", days: 182, bucket: "week" },
  { id: "year", label: "Anual", days: 365, bucket: "month" },
];

const PLATFORM_LABEL: Record<DownloadPlatform, string> = {
  windows: "Windows",
  portable: "Windows · Portátil",
  appimage: "Linux · AppImage",
  deb: "Linux · .deb",
  android: "Android · Lite",
};
const PLATFORMS = Object.keys(PLATFORM_LABEL) as DownloadPlatform[];
const MONTHS = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];

/** Hoje no horário de Brasília (o backend grava os dias nesse fuso). */
function todayBrasilia(): string {
  return new Intl.DateTimeFormat("en-CA", { timeZone: "America/Sao_Paulo" }).format(new Date());
}
function addDays(day: string, n: number): string {
  const d = new Date(`${day}T12:00:00Z`);
  d.setUTCDate(d.getUTCDate() + n);
  return d.toISOString().slice(0, 10);
}
const shortDate = (day: string) => `${day.slice(8, 10)}/${day.slice(5, 7)}`;

interface Bucket {
  key: string;
  label: string;
  /** Texto completo do intervalo, para o tooltip. */
  range: string;
  days: string[];
}

/** Barras do período: um dia, uma semana (7 dias) ou um mês do calendário. */
function bucketsFor(period: (typeof PERIODS)[number], end: string): Bucket[] {
  const start = addDays(end, -(period.days - 1));
  const all = Array.from({ length: period.days }, (_, i) => addDays(start, i));
  if (period.bucket === "day") {
    return all.map((d) => ({ key: d, label: shortDate(d), range: d.split("-").reverse().join("/"), days: [d] }));
  }
  if (period.bucket === "week") {
    const out: Bucket[] = [];
    for (let i = 0; i < all.length; i += 7) {
      const days = all.slice(i, i + 7);
      out.push({ key: days[0], label: shortDate(days[0]), range: `${shortDate(days[0])} a ${shortDate(days.at(-1)!)}`, days });
    }
    return out;
  }
  const byMonth = new Map<string, string[]>();
  for (const d of all) byMonth.set(d.slice(0, 7), [...(byMonth.get(d.slice(0, 7)) ?? []), d]);
  return [...byMonth].map(([month, days]) => {
    const label = `${MONTHS[Number(month.slice(5, 7)) - 1]}/${month.slice(2, 4)}`;
    return { key: month, label, range: `${label} (${shortDate(days[0])} a ${shortDate(days.at(-1)!)})`, days };
  });
}

interface Totals {
  clicks: number;
  people: number;
  byPlatform: Record<DownloadPlatform, number>;
}

function totalsFor(data: ClientOverview, days: Set<string>): Totals {
  const byPlatform = Object.fromEntries(PLATFORMS.map((p) => [p, 0])) as Record<DownloadPlatform, number>;
  for (const row of data.days) {
    if (!days.has(row.day)) continue;
    for (const p of PLATFORMS) byPlatform[p] += row.clicks[p] ?? 0;
  }
  const clicks = PLATFORMS.reduce((sum, p) => sum + byPlatform[p], 0);
  const people = data.visitors.filter((visited) => visited.some((d) => days.has(d))).length;
  return { clicks, people, byPlatform };
}

/** "+25%" / "−10%" / "novo" em relação ao período anterior. */
function change(now: number, before: number): { text: string; tone: "success" | "danger" | "neutral" } {
  if (before === 0) return now === 0 ? { text: "sem mudança", tone: "neutral" } : { text: "novo", tone: "success" };
  const pct = Math.round(((now - before) / before) * 100);
  if (pct === 0) return { text: "igual", tone: "neutral" };
  return { text: `${pct > 0 ? "+" : "−"}${Math.abs(pct)}% vs. anterior`, tone: pct > 0 ? "success" : "danger" };
}

/** Client Manager: interesse no ShmupX pelos cliques nos instaladores do site e versões em uso. */
export default function ClientView() {
  const [data, setData] = useState<ClientOverview | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [periodId, setPeriodId] = useState<Period>("month");

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      setData(await getClientOverview());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);
  useEffect(() => {
    void reload();
    const timer = window.setInterval(() => void reload(), 60_000);
    return () => window.clearInterval(timer);
  }, [reload]);

  const period = PERIODS.find((p) => p.id === periodId)!;
  const view = useMemo(() => {
    if (!data) return null;
    const end = todayBrasilia();
    const buckets = bucketsFor(period, end);
    const current = new Set(buckets.flatMap((b) => b.days));
    const previousEnd = addDays(end, -period.days);
    const previous = new Set(Array.from({ length: period.days }, (_, i) => addDays(previousEnd, -i)));
    const rows = buckets.map((b) => {
      const t = totalsFor(data, new Set(b.days));
      return { ...b, clicks: t.clicks, people: t.people };
    });
    return { rows, now: totalsFor(data, current), before: totalsFor(data, previous), first: data.days[0]?.day ?? null };
  }, [data, period]);

  const latest = data?.downloads.windows?.version;

  return (
    <section className="flex min-w-0 flex-1 flex-col overflow-auto">
      <header className="flex flex-wrap items-end justify-between gap-4 px-8 pt-7 pb-5">
        <div>
          <h1 className="text-xl font-semibold text-zinc-100">Client Manager</h1>
          <p className="mt-1 max-w-2xl text-sm text-zinc-500">
            Interesse no ShmupX: cliques nos instaladores da página de Download do site e quantas pessoas diferentes clicaram. Embaixo, as versões do Client que os jogadores usam.
          </p>
        </div>
        <Button variant="ghost" onClick={() => void reload()} disabled={loading} title="Recarregar" aria-label="Recarregar">
          <IconRefresh className={loading ? "animate-spin" : ""} />
        </Button>
      </header>

      {error && (
        <p role="alert" className="mx-8 mb-4 flex gap-2 rounded-md border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
          <IconAlert className="mt-0.5 flex-shrink-0" />
          {error}
        </p>
      )}

      <div className="mx-8 mb-5 flex flex-wrap items-center gap-3">
        <div role="radiogroup" aria-label="Período do dashboard" className="flex gap-1 rounded-lg border border-white/10 bg-zinc-950 p-1">
          {PERIODS.map((p) => (
            <button key={p.id} type="button" role="radio" aria-checked={periodId === p.id} onClick={() => setPeriodId(p.id)}
              className={`rounded-md px-3 py-1 text-xs font-medium transition-colors focus-visible:ring-2 focus-visible:ring-cyan-400/50 focus-visible:outline-none ${periodId === p.id ? "bg-cyan-400 text-zinc-950" : "text-zinc-400 hover:bg-white/5 hover:text-zinc-100"}`}>
              {p.label}
            </button>
          ))}
        </div>
        <span className="text-xs text-zinc-500">
          Últimos {period.days} dias até hoje · horário de Brasília{view?.first ? ` · contagem desde ${view.first.split("-").reverse().join("/")}` : ""}
        </span>
      </div>

      {!view ? (
        <div className="mx-8 flex min-h-56 flex-col items-center justify-center gap-2 rounded-lg border border-white/5 bg-zinc-950/40 text-center">
          <IconMonitor className="h-6 w-6 text-zinc-600" />
          <p className="text-sm text-zinc-300">{loading ? "Carregando…" : "Sem dados"}</p>
        </div>
      ) : (
        <div className="mx-8 mb-8 space-y-5">
          <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
            <Kpi label="Cliques nos instaladores" value={view.now.clicks} change={change(view.now.clicks, view.before.clicks)} />
            <Kpi label="Pessoas diferentes" value={view.now.people} change={change(view.now.people, view.before.people)} hint="visitantes anônimos que clicaram ao menos uma vez" />
            <Kpi label="Cliques por pessoa" value={view.now.people ? Math.round((view.now.clicks / view.now.people) * 10) / 10 : 0} hint="média no período" />
          </div>

          <div className="grid grid-cols-1 gap-5 xl:grid-cols-[1fr_320px]">
            <div className="space-y-5">
              <BarChart title="Cliques nos instaladores" unit="clique" rows={view.rows.map((r) => ({ key: r.key, label: r.label, range: r.range, value: r.clicks }))} />
              <BarChart title="Pessoas diferentes que clicaram" unit="pessoa" rows={view.rows.map((r) => ({ key: r.key, label: r.label, range: r.range, value: r.people }))} />
            </div>
            <div className="space-y-5">
              <PlatformBars totals={view.now} />
              <Versions versions={data!.versions} latest={latest} />
            </div>
          </div>
        </div>
      )}
    </section>
  );
}

function Kpi({ label, value, change: delta, hint }: { label: string; value: number; change?: { text: string; tone: "success" | "danger" | "neutral" }; hint?: string }) {
  return (
    <div className="rounded-lg border border-white/5 bg-zinc-950/60 px-5 py-4">
      <p className="text-[11px] font-semibold tracking-wider text-zinc-500 uppercase">{label}</p>
      <p className="mt-1 flex items-baseline gap-3">
        <span className="text-3xl font-semibold text-zinc-100 tabular-nums">{value.toLocaleString("pt-BR")}</span>
        {delta && <Badge tone={delta.tone}>{delta.text}</Badge>}
      </p>
      {hint && <p className="mt-1 text-[11px] text-zinc-600">{hint}</p>}
    </div>
  );
}

/** Barras verticais de uma série só, com eixo recessivo e tooltip por barra. */
function BarChart({ title, unit, rows }: { title: string; unit: string; rows: { key: string; label: string; range: string; value: number }[] }) {
  const [hover, setHover] = useState<number | null>(null);
  const max = Math.max(1, ...rows.map((r) => r.value));
  // Escala "redonda" para a linha de grade do topo.
  const step = 10 ** Math.floor(Math.log10(max));
  const top = Math.ceil(max / step) * step;
  const every = Math.ceil(rows.length / 12);
  const plural = (n: number) => `${n.toLocaleString("pt-BR")} ${n === 1 ? unit : `${unit}s`}`;
  const total = rows.reduce((s, r) => s + r.value, 0);
  return (
    <figure className="rounded-lg border border-white/5 bg-zinc-950/60 p-5">
      <figcaption className="mb-4 flex items-baseline justify-between gap-3">
        <span className="text-sm font-medium text-zinc-200">{title}</span>
        <span className="text-xs text-zinc-500 tabular-nums">{plural(total)} no período</span>
      </figcaption>
      <div className="relative flex h-44 gap-3">
        <div className="flex w-8 flex-col justify-between text-right text-[10px] text-zinc-600 tabular-nums" aria-hidden>
          <span>{top.toLocaleString("pt-BR")}</span>
          <span>{(top / 2).toLocaleString("pt-BR")}</span>
          <span>0</span>
        </div>
        <div className="relative flex-1">
          <div className="pointer-events-none absolute inset-0 flex flex-col justify-between" aria-hidden>
            <span className="border-t border-white/5" /><span className="border-t border-white/5" /><span className="border-t border-white/10" />
          </div>
          <div className="absolute inset-0 flex items-end gap-0.5" role="list" aria-label={title}>
            {rows.map((r, i) => (
              <div key={r.key} role="listitem" aria-label={`${r.range}: ${plural(r.value)}`}
                className="group relative flex h-full flex-1 items-end justify-center"
                onMouseEnter={() => setHover(i)} onMouseLeave={() => setHover(null)}>
                <div className={`w-full max-w-10 rounded-t-[4px] transition-colors ${hover === i ? "bg-cyan-300" : "bg-cyan-400/80"}`}
                  style={{ height: r.value ? `${Math.max(2, (r.value / top) * 100)}%` : 0 }} />
                {hover === i && (
                  <div className="pointer-events-none absolute bottom-full z-10 mb-2 rounded-md border border-white/10 bg-zinc-900 px-2.5 py-1.5 text-xs whitespace-nowrap text-zinc-100 shadow-lg shadow-black/50">
                    <span className="block text-[10px] text-zinc-500">{r.range}</span>
                    {plural(r.value)}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      </div>
      <div className="mt-2 ml-11 flex gap-0.5 text-[10px] text-zinc-600" aria-hidden>
        {rows.map((r, i) => <span key={r.key} className="flex-1 truncate text-center">{i % every === 0 ? r.label : ""}</span>)}
      </div>
    </figure>
  );
}

/** Cliques por instalador: barras horizontais com o nome e o número em texto. */
function PlatformBars({ totals }: { totals: Totals }) {
  const max = Math.max(1, ...PLATFORMS.map((p) => totals.byPlatform[p]));
  return (
    <div className="rounded-lg border border-white/5 bg-zinc-950/60 p-5">
      <p className="mb-4 text-sm font-medium text-zinc-200">Por instalador</p>
      <ul className="space-y-3">
        {[...PLATFORMS].sort((a, b) => totals.byPlatform[b] - totals.byPlatform[a]).map((p) => {
          const n = totals.byPlatform[p];
          const pct = totals.clicks ? Math.round((n / totals.clicks) * 100) : 0;
          return (
            <li key={p}>
              <div className="mb-1 flex justify-between text-xs">
                <span className="text-zinc-300">{PLATFORM_LABEL[p]}</span>
                <span className="text-zinc-400 tabular-nums">{n.toLocaleString("pt-BR")} <span className="text-zinc-600">· {pct}%</span></span>
              </div>
              <div className="h-2 rounded-full bg-white/5">
                <div className="h-2 rounded-full bg-cyan-400/80" style={{ width: `${(n / max) * 100}%` }} />
              </div>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

function Versions({ versions, latest }: { versions: ClientOverview["versions"]; latest?: string }) {
  const total = versions.reduce((s, v) => s + v.players, 0);
  return (
    <div className="rounded-lg border border-white/5 bg-zinc-950/60 p-5">
      <p className="mb-1 text-sm font-medium text-zinc-200">Versões do Client em uso</p>
      <p className="mb-3 text-[11px] text-zinc-600">Versão de cada jogador no último acesso · ativos = usaram nos últimos 7 dias</p>
      {versions.length === 0 ? (
        <p className="text-xs text-zinc-500">Nenhum jogador ainda.</p>
      ) : (
        <table className="w-full text-left text-xs">
          <thead className="text-[10px] tracking-wider text-zinc-600 uppercase">
            <tr><th className="pb-2 font-semibold">Versão</th><th className="pb-2 text-right font-semibold">Jogadores</th><th className="pb-2 text-right font-semibold">Ativos</th></tr>
          </thead>
          <tbody className="divide-y divide-white/5">
            {versions.map((v) => (
              <tr key={v.version}>
                <td className="py-1.5 font-mono text-zinc-200">
                  {v.version} {v.version === latest && <Badge tone="info">mais nova</Badge>}
                </td>
                <td className="py-1.5 text-right text-zinc-300 tabular-nums">{v.players} <span className="text-zinc-600">· {total ? Math.round((v.players / total) * 100) : 0}%</span></td>
                <td className="py-1.5 text-right text-zinc-400 tabular-nums">{v.active_7d}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
