import type { PontuacaoBruta } from "../types/ranking";

const NOMES_MOCK = ["ZANKI", "KINEKO", "SHIGEI", "FURUYA", "NOVAX", "YUMEKO"];

/** Hash simples e determinístico — mesma `seed` sempre gera o mesmo placar mock. */
function hashSeed(seed: string): number {
  let hash = 0;
  for (let i = 0; i < seed.length; i += 1) {
    hash = (hash * 31 + seed.charCodeAt(i)) >>> 0;
  }
  return hash;
}

/**
 * Placar bruto mock para a tela de detalhes de um jogo — o backend ainda não tem um endpoint
 * `GET /api/jogos/:id/scores` (ver TASKS.md), então isto é só para a UI ter algo real para
 * desenhar. Determinístico por `seed` (ex.: o `id` do jogo) para não mudar a cada reload.
 */
export function gerarPlacarMock(seed: string): PontuacaoBruta[] {
  const base = hashSeed(seed);

  return NOMES_MOCK.map((nome, index) => {
    const variacao = (base + index * 7919) % 50_000;
    const scoreBruto = 500_000 - index * 60_000 + variacao;
    const vidasPerdidas = (base + index * 3) % 5;
    const continuesUsados = index === NOMES_MOCK.length - 1 ? 1 : 0;

    return { jogadorNome: nome, scoreBruto, vidasPerdidas, continuesUsados };
  }).sort((a, b) => b.scoreBruto - a.scoreBruto);
}
