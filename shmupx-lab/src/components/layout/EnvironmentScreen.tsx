import type { Ambiente } from "../../lib/ambiente";

interface EnvironmentScreenProps {
  /** Última escolha, destacada e com foco (Enter confirma). */
  preselected: Ambiente | null;
  onChoose: (ambiente: Ambiente) => void;
}

const OPCOES: { id: Ambiente; label: string; detalhe: string; cor: string; corAtiva: string }[] = [
  {
    id: "dev",
    label: "Dev (emuladores locais)",
    detalhe: "Firebase nos emuladores do Docker e backend em localhost. Nada chega aos jogadores.",
    cor: "border-amber-400/30 bg-amber-500/5 text-amber-100 hover:bg-amber-500/15",
    corAtiva: "ring-2 ring-amber-400/60 bg-amber-500/15",
  },
  {
    id: "prd",
    label: "Prod",
    detalhe: "Projeto Firebase real e backend no Cloud Run. Mudanças valem para os jogadores.",
    cor: "border-red-500/30 bg-red-500/5 text-red-100 hover:bg-red-500/15",
    corAtiva: "ring-2 ring-red-400/60 bg-red-500/15",
  },
];

/** Primeira tela do Lab: escolhe o ambiente antes de iniciar o Firebase e o login. */
export default function EnvironmentScreen({ preselected, onChoose }: EnvironmentScreenProps) {
  return (
    <main className="flex h-screen w-screen items-center justify-center bg-zinc-950 text-zinc-100">
      <div className="w-96 rounded-lg border border-white/5 bg-zinc-900/50 p-8 text-center">
        <p className="text-xs font-semibold uppercase tracking-[0.25em] text-cyan-400">ShmupX Lab</p>
        <h1 className="mt-3 text-lg font-semibold">Escolha o ambiente</h1>
        <p className="mt-2 text-sm text-zinc-400">Para trocar depois, use "Trocar ambiente" no menu.</p>
        <div className="mt-6 space-y-3">
          {OPCOES.map(({ id, label, detalhe, cor, corAtiva }) => {
            const ultimo = id === preselected;
            return (
              <button
                key={id}
                type="button"
                autoFocus={ultimo}
                onClick={() => onChoose(id)}
                className={`w-full rounded-md border px-4 py-3 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cyan-400/50 ${cor} ${ultimo ? corAtiva : ""}`}
              >
                <span className="flex items-center justify-between gap-2">
                  <span className="text-sm font-medium">{label}</span>
                  {ultimo && <span className="text-[10px] uppercase tracking-wider text-zinc-400">último usado</span>}
                </span>
                <span className="mt-1 block text-xs leading-snug text-zinc-400">{detalhe}</span>
              </button>
            );
          })}
        </div>
      </div>
    </main>
  );
}