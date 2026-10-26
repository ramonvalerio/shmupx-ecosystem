/** Espelha o JSON de `GET /api/jogos` do shmupx-backend (serde default: snake_case). */
export interface Jogo {
  id: string;
  steam_app_id: number;
  nome: string;
  criado_em: string;
}
