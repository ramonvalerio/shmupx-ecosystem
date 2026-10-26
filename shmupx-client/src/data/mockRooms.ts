import type { ChatMessage, GameRoom } from "../types/room";

export const MOCK_ROOMS: GameRoom[] = [
  { id: "mushihimesama", title: "Mushihimesama", playersOnline: 128, steamAppId: 1358040 },
  { id: "dodonpachi", title: "DoDonPachi", playersOnline: 74, steamAppId: 1358050 },
  { id: "crimson-clover", title: "Crimson Clover", playersOnline: 41, steamAppId: 452480 },
];

export const MOCK_MESSAGES: Record<string, ChatMessage[]> = {
  mushihimesama: [
    { id: "m1", author: "ZANKI", text: "alguém topa rank hoje à noite?", timestamp: "21:02" },
    { id: "m2", author: "KINEKO", text: "entro em 10min, bora", timestamp: "21:03" },
    {
      id: "m3",
      author: "RobotX",
      text: "Lembrete: sua ficha só é consumida quando a run começa a valer.",
      timestamp: "21:04",
    },
  ],
  dodonpachi: [
    { id: "d1", author: "SHIGEI", text: "novo recorde na stage 3!", timestamp: "19:40" },
    { id: "d2", author: "FURUYA", text: "manda o replay", timestamp: "19:41" },
  ],
  "crimson-clover": [{ id: "c1", author: "NOVAX", text: "sala calma hoje", timestamp: "18:12" }],
};
