import type { JogadorRanking } from "../../types/ranking";
import PlayerAvatar from "./PlayerAvatar";

const ESTILO_POSICAO: Record<number, string> = {
  1: "border-amber-400/60 bg-amber-400/10 text-amber-300 shadow-[0_0_24px_-6px_rgba(251,191,36,0.6)]",
  2: "border-zinc-300/50 bg-zinc-300/10 text-zinc-200",
  3: "border-orange-500/50 bg-orange-500/10 text-orange-300",
};

interface GlobalLeaderboardTableProps {
  jogadores: JogadorRanking[];
}

export default function GlobalLeaderboardTable({ jogadores }: GlobalLeaderboardTableProps) {
  return (
    <div className="overflow-hidden rounded-lg border border-white/10">
      <table className="w-full border-collapse text-left text-sm">
        <thead>
          <tr className="border-b border-white/10 bg-zinc-900/60 text-[11px] uppercase tracking-wider text-zinc-500">
            <th className="px-4 py-3 font-semibold">Rank</th>
            <th className="px-4 py-3 font-semibold">Jogador</th>
            <th className="px-4 py-3 font-semibold">Média Global</th>
            <th className="hidden px-4 py-3 font-semibold sm:table-cell">Jogos Principais</th>
          </tr>
        </thead>
        <tbody>
          {jogadores.map((jogador, index) => {
            const posicao = index + 1;
            const estiloTop3 = ESTILO_POSICAO[posicao];

            return (
              <tr
                key={jogador.id}
                className={`border-b border-white/5 transition-colors ${
                  estiloTop3 ? "" : "hover:bg-white/5"
                }`}
              >
                <td className="px-4 py-3">
                  <span
                    className={`inline-flex h-7 w-7 items-center justify-center rounded-md border font-arcade text-xs ${
                      estiloTop3 ?? "border-white/10 text-zinc-400"
                    }`}
                  >
                    {posicao}
                  </span>
                </td>
                <td className="px-4 py-3">
                  <div className="flex items-center gap-3">
                    <PlayerAvatar iniciais={jogador.avatarIniciais} seed={jogador.id} />
                    <span className="font-medium text-zinc-100">{jogador.nome}</span>
                  </div>
                </td>
                <td className="px-4 py-3">
                  <span
                    className={`font-arcade text-sm ${
                      estiloTop3 ? "text-amber-300" : "text-emerald-300"
                    }`}
                  >
                    {jogador.pontuacaoGlobalNormalizada.toFixed(2)}
                  </span>
                </td>
                <td className="hidden px-4 py-3 text-xs text-zinc-400 sm:table-cell">
                  {jogador.jogosPrincipais.join(" · ")}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
