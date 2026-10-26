/** Linha do ranking online de uma sala (mockado por ora — sem GET /api/jogos/:id/scores ainda). */
export interface RoomRankingEntry {
  jogadorNome: string;
  maiorPlacarBruto: number;
  vidasPerdidas: number;
  continuesUsados: number;
  notaNormalizada: number;
}
