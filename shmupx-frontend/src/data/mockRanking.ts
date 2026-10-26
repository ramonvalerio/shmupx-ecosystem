import type { JogadorRanking } from "../types/ranking";

/** Mock do leaderboard global — ver nota em types/ranking.ts sobre o que falta no backend. */
export const MOCK_RANKING: JogadorRanking[] = [
  {
    id: "zanki",
    nome: "ZANKI",
    avatarIniciais: "ZK",
    pontuacaoGlobalNormalizada: 9.42,
    jogosPrincipais: ["Mushihimesama", "DoDonPachi Resurrection", "Crimson Clover"],
  },
  {
    id: "kineko",
    nome: "KINEKO",
    avatarIniciais: "KN",
    pontuacaoGlobalNormalizada: 8.87,
    jogosPrincipais: ["DoDonPachi Resurrection", "Mushihimesama"],
  },
  {
    id: "shigei",
    nome: "SHIGEI",
    avatarIniciais: "SG",
    pontuacaoGlobalNormalizada: 8.21,
    jogosPrincipais: ["Crimson Clover", "DoDonPachi Resurrection", "Mushihimesama"],
  },
  {
    id: "furuya",
    nome: "FURUYA",
    avatarIniciais: "FR",
    pontuacaoGlobalNormalizada: 7.65,
    jogosPrincipais: ["Mushihimesama"],
  },
  {
    id: "novax",
    nome: "NOVAX",
    avatarIniciais: "NV",
    pontuacaoGlobalNormalizada: 6.98,
    jogosPrincipais: ["Crimson Clover"],
  },
  {
    id: "robotx-tutor",
    nome: "RX_TUTOR",
    avatarIniciais: "RX",
    pontuacaoGlobalNormalizada: 6.40,
    jogosPrincipais: ["DoDonPachi Resurrection"],
  },
  {
    id: "yumeko",
    nome: "YUMEKO",
    avatarIniciais: "YM",
    pontuacaoGlobalNormalizada: 5.75,
    jogosPrincipais: ["Mushihimesama", "Crimson Clover"],
  },
  {
    id: "daichi",
    nome: "DAICHI",
    avatarIniciais: "DC",
    pontuacaoGlobalNormalizada: 5.10,
    jogosPrincipais: ["DoDonPachi Resurrection"],
  },
];
