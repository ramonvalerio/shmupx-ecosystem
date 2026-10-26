import type { RoomRankingEntry } from "../types/ranking";

const NOMES_MOCK = ["ZANKI", "KINEKO", "SHIGEI", "FURUYA", "NOVAX"];

/** Hash simples e determinístico — mesma `seed` sempre gera o mesmo ranking mock. */
function hashSeed(seed: string): number {
  let hash = 0;
  for (let i = 0; i < seed.length; i += 1) {
    hash = (hash * 31 + seed.charCodeAt(i)) >>> 0;
  }
  return hash;
}

/**
 * Ranking online mockado para a gaveta de uma sala — sem `GET /api/jogos/:id/scores` no
 * backend ainda (ver shmupx-backend/TASKS.md). Determinístico por `seed` (o id da sala).
 */
export function gerarRankingSalaMock(seed: string): RoomRankingEntry[] {
  const base = hashSeed(seed);

  return NOMES_MOCK.map((nome, index) => {
    const variacao = (base + index * 7919) % 50_000;
    const maiorPlacarBruto = 400_000 - index * 55_000 + variacao;
    const vidasPerdidas = (base + index * 3) % 5;
    const continuesUsados = index >= NOMES_MOCK.length - 2 ? 1 : 0;
    const notaBase = 9.0 - index * 1.3;
    const notaNormalizada = Math.max(1, notaBase - continuesUsados * (notaBase * 0.9));

    return {
      jogadorNome: nome,
      maiorPlacarBruto,
      vidasPerdidas,
      continuesUsados,
      notaNormalizada: Math.round(notaNormalizada * 100) / 100,
    };
  }).sort((a, b) => b.notaNormalizada - a.notaNormalizada);
}
