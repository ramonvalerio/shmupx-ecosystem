import Link from "next/link";
import { notFound } from "next/navigation";
import { getJogos } from "../../../services/api";
import { MOCK_JOGOS } from "../../../data/mockJogos";
import { gerarPlacarMock } from "../../../data/mockScores";
import ScoreTable from "../../../components/jogos/ScoreTable";

interface JogoDetailContentProps {
  paramsPromise: Promise<{ jogoId: string }>;
}

/**
 * Componente async separado da page: sob Cache Components, `await params` (dinâmico) e o
 * fetch sem cache precisam ficar atrás de um `<Suspense>`, não no topo da própria page.
 */
export default async function JogoDetailContent({ paramsPromise }: JogoDetailContentProps) {
  const { jogoId } = await paramsPromise;

  let jogos;
  try {
    jogos = await getJogos();
  } catch (error) {
    console.warn("JogoDetailContent: falha ao buscar GET /api/jogos, usando mock:", error);
    jogos = MOCK_JOGOS;
  }

  const jogo = jogos.find((candidato) => candidato.id === jogoId);
  if (!jogo) {
    notFound();
  }

  // Sem GET /api/jogos/:id/scores no backend ainda — placar mock determinístico por jogo.id.
  const placar = gerarPlacarMock(jogo.id);

  return (
    <>
      <div className="mb-6">
        <Link
          href="/jogos"
          className="text-xs uppercase tracking-wider text-cyan-400 transition-colors hover:text-cyan-300"
        >
          ← Voltar para Jogos
        </Link>
        <p className="mt-3 text-xs uppercase tracking-wider text-zinc-500">
          Steam App ID {jogo.steam_app_id}
        </p>
        <h1 className="mt-1 text-2xl font-semibold text-zinc-100">{jogo.nome}</h1>
      </div>

      <ScoreTable placar={placar} />
      <p className="mt-3 text-center text-xs text-amber-400/80">
        Placar de exemplo — o backend ainda não tem {"GET /api/jogos/:id/scores"} (ver
        shmupx-frontend/TASKS.md).
      </p>
    </>
  );
}
