import { getJogos } from "../../services/api";
import { MOCK_JOGOS } from "../../data/mockJogos";
import JogoCard from "./JogoCard";

/**
 * Busca real em `GET /api/jogos` (ver services/api.ts). Se o backend Axum não estiver no ar
 * (comum em dev sem `cargo run` do shmupx-backend ativo), cai para os jogos mockados em vez de
 * quebrar a página inteira — a integração real já está pronta, só falta o backend estar de pé.
 */
export default async function JogosList() {
  let jogos;
  let usandoMock = false;

  try {
    jogos = await getJogos();
  } catch (error) {
    console.warn("JogosList: falha ao buscar GET /api/jogos, usando mock:", error);
    jogos = MOCK_JOGOS;
    usandoMock = true;
  }

  if (jogos.length === 0) {
    return (
      <p className="rounded-lg border border-dashed border-white/10 p-8 text-center text-sm text-zinc-500">
        Nenhum jogo cadastrado ainda.
      </p>
    );
  }

  return (
    <div>
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {jogos.map((jogo) => (
          <JogoCard key={jogo.id} jogo={jogo} />
        ))}
      </div>
      {usandoMock && (
        <p className="mt-4 text-center text-xs text-amber-400/80">
          Não foi possível conectar ao backend ({"GET /api/jogos"}) — exibindo dados de exemplo.
        </p>
      )}
    </div>
  );
}
