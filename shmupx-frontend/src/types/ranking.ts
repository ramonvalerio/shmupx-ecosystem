/**
 * Linha do ranking global. Mockado por ora: o backend ainda não tem um endpoint que junte
 * `perfis_jogadores.pontuacao_global_normalizada` com nome/avatar de exibição (a tabela só
 * guarda o UUID de `auth.users`, sem `display_name`/`avatar_url`) nem uma agregação de "top 3
 * jogos". Ver shmupx-frontend/TASKS.md para o que falta no backend para isto virar dado real.
 */
export interface JogadorRanking {
  id: string;
  nome: string;
  avatarIniciais: string;
  pontuacaoGlobalNormalizada: number;
  jogosPrincipais: string[];
}

/** Linha de placar bruto por jogo. Mockado por ora (sem endpoint `GET /api/jogos/:id/scores`). */
export interface PontuacaoBruta {
  jogadorNome: string;
  scoreBruto: number;
  vidasPerdidas: number;
  continuesUsados: number;
}
