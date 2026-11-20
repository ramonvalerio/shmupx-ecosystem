import { useEffect, useState } from "react";
import { collection, onSnapshot, query, where } from "firebase/firestore";
import { db } from "../auth/firebase";
import type { GameRoom } from "../types/room";

const texto = (value: unknown) => (typeof value === "string" && value.trim() ? value.trim() : undefined);
const COVER_PREFIX = "https://www.shmupx.com/catalog/covers/";

/** Documento `games/{gameId}` publicado pelo Lab (shmupx-backend, rota de admin) -> sala. */
export function roomFromGame(id: string, data: Record<string, unknown>): GameRoom | null {
  if (typeof data.title !== "string" || !data.title) return null;
  return {
    id,
    title: data.title,
    // Sem presença em tempo real ainda: as telas rotulam este número como demonstração.
    playersOnline: 0,
    steamAppId: typeof data.steamAppId === "number" ? data.steamAppId : 0,
    // Só capas do próprio site (o backend já recusa outras origens).
    coverUrl: typeof data.coverUrl === "string" && data.coverUrl.startsWith(COVER_PREFIX) ? data.coverUrl : undefined,
    platform: texto(data.platform),
    developer: texto(data.developer),
    publisher: texto(data.publisher),
    genre: texto(data.genre),
    year: typeof data.year === "number" ? data.year : undefined,
  };
}

/**
 * Catálogo de jogos = documentos `games` com `published == true` (ADR 006: um jogo, uma sala).
 * A consulta repete o filtro das Security Rules, que só liberam leitura dos publicados. Escuta em
 * tempo real: publicar ou despublicar no Lab aparece aqui sem reiniciar o app.
 */
export function useCatalog(enabled: boolean): GameRoom[] {
  const [rooms, setRooms] = useState<GameRoom[]>([]);

  useEffect(() => {
    if (!enabled) { setRooms([]); return; }
    return onSnapshot(
      query(collection(db, "games"), where("published", "==", true)),
      (snapshot) => {
        const games = snapshot.docs
          .map((doc) => roomFromGame(doc.id, doc.data()))
          .filter((room): room is GameRoom => room !== null)
          .sort((a, b) => a.title.localeCompare(b.title));
        setRooms(games);
      },
      (error) => console.error("falha ao carregar o catálogo de jogos:", error),
    );
  }, [enabled]);

  return rooms;
}
