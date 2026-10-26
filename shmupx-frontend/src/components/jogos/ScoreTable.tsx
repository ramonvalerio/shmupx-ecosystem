import type { PontuacaoBruta } from "../../types/ranking";

interface ScoreTableProps {
  placar: PontuacaoBruta[];
}

export default function ScoreTable({ placar }: ScoreTableProps) {
  return (
    <div className="overflow-hidden rounded-lg border border-white/10">
      <table className="w-full border-collapse text-left text-sm">
        <thead>
          <tr className="border-b border-white/10 bg-zinc-900/60 text-[11px] uppercase tracking-wider text-zinc-500">
            <th className="px-4 py-3 font-semibold">#</th>
            <th className="px-4 py-3 font-semibold">Jogador</th>
            <th className="px-4 py-3 font-semibold">Score Bruto</th>
            <th className="px-4 py-3 font-semibold">Vidas Perdidas</th>
            <th className="px-4 py-3 font-semibold">Continues</th>
          </tr>
        </thead>
        <tbody>
          {placar.map((entrada, index) => (
            <tr
              key={entrada.jogadorNome}
              className="border-b border-white/5 last:border-b-0 hover:bg-white/5"
            >
              <td className="px-4 py-3 font-arcade text-xs text-zinc-500">{index + 1}</td>
              <td className="px-4 py-3 font-medium text-zinc-100">{entrada.jogadorNome}</td>
              <td className="px-4 py-3 font-arcade text-sm text-emerald-300">
                {entrada.scoreBruto.toLocaleString("pt-BR")}
              </td>
              <td className="px-4 py-3 text-zinc-400">{entrada.vidasPerdidas}</td>
              <td className="px-4 py-3">
                {entrada.continuesUsados > 0 ? (
                  <span className="rounded-full border border-amber-500/40 bg-amber-500/10 px-2 py-0.5 text-[11px] text-amber-300">
                    {entrada.continuesUsados} continue
                  </span>
                ) : (
                  <span className="text-[11px] text-zinc-600">1CC</span>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
