import type { Jogo } from "../types/jogo";

/** Usado só quando `GET /api/jogos` falha (backend fora do ar) — ver services/api.ts. */
export const MOCK_JOGOS: Jogo[] = [
  {
    id: "00000000-0000-0000-0000-000000000001",
    steam_app_id: 1358040,
    nome: "Mushihimesama",
    criado_em: new Date().toISOString(),
  },
  {
    id: "00000000-0000-0000-0000-000000000002",
    steam_app_id: 1358050,
    nome: "DoDonPachi Resurrection",
    criado_em: new Date().toISOString(),
  },
  {
    id: "00000000-0000-0000-0000-000000000003",
    steam_app_id: 452480,
    nome: "Crimson Clover",
    criado_em: new Date().toISOString(),
  },
];
