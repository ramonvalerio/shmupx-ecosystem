import GlobalLeaderboardTable from "../components/leaderboard/GlobalLeaderboardTable";
import PodiumTop3 from "../components/leaderboard/PodiumTop3";
import { MOCK_RANKING } from "../data/mockRanking";

export default function HomePage() {
  const ranking = [...MOCK_RANKING].sort(
    (a, b) => b.pontuacaoGlobalNormalizada - a.pontuacaoGlobalNormalizada,
  );

  return (
    <div className="mx-auto max-w-5xl px-4 py-10 sm:px-6">
      <div className="mb-10 text-center">
        <p className="font-arcade text-[11px] uppercase tracking-[0.3em] text-fuchsia-400">
          Temporada Atual
        </p>
        <h1 className="mt-3 font-arcade text-2xl text-amber-300 sm:text-3xl">
          TOP 3 LEADERBOARD
        </h1>
        <p className="mx-auto mt-3 max-w-xl text-sm text-zinc-400">
          Ranking global normalizado (0.00–10.00) por desempenho relativo em cada shmup —
          comparável entre jogos diferentes, calculado por Z-score na temporada.
        </p>
      </div>

      <PodiumTop3 top3={ranking.slice(0, 3)} />

      <div className="mt-10">
        <h2 className="mb-3 text-xs font-semibold uppercase tracking-wider text-zinc-500">
          Ranking Completo
        </h2>
        <GlobalLeaderboardTable jogadores={ranking} />
        <p className="mt-3 text-center text-xs text-zinc-600">
          Dados de exemplo — o backend ainda não expõe um endpoint de ranking global agregado
          (nome/avatar/top jogos por jogador). Ver shmupx-frontend/TASKS.md.
        </p>
      </div>
    </div>
  );
}
