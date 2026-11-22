import type { GameRoom } from "../types/room";
import type { SessionRun } from "../types/submission";

export interface CollectionPreferences {
  view: "grid" | "list";
  sort: "name" | "favorites" | "developer";
  descending: boolean;
  favorites: string[];
}
export function defaultCollection(): CollectionPreferences { return { view: "grid", sort: "name", descending: false, favorites: [] }; }
export function validateCollection(value: unknown): CollectionPreferences {
  if (!value || typeof value !== "object") return defaultCollection();
  const saved = value as Partial<CollectionPreferences>;
  return {
    view: saved.view === "list" ? "list" : "grid",
    sort: saved.sort === "favorites" || saved.sort === "developer" ? saved.sort : "name",
    descending: saved.descending === true,
    favorites: Array.isArray(saved.favorites) ? [...new Set(saved.favorites.filter((id): id is string => typeof id === "string" && id.length > 0 && id.length <= 128))].slice(0, 2000) : [],
  };
}
export function selectCollection(rooms: GameRoom[], runs: SessionRun[], preferences: CollectionPreferences, query: string, filter: string): GameRoom[] {
  const normalize = (text: string) => text.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLocaleLowerCase("pt-BR").trim();
  const search = normalize(query);
  const favorites = new Set(preferences.favorites);
  const recent = new Map<string, number>();
  for (const run of runs) recent.set(run.gameId, Math.max(recent.get(run.gameId) ?? 0, run.startedAt));
  return rooms.filter((room) => normalize(room.title).includes(search) && (filter !== "favorites" || favorites.has(room.id)) && (filter !== "recorded" || recent.has(room.id))).sort((a, b) => {
    if (preferences.sort === "favorites") { const delta = Number(favorites.has(b.id)) - Number(favorites.has(a.id)); if (delta) return delta; }
    // Por desenvolvedora (A–Z); sem desenvolvedora no catálogo vai para o fim. Empate: pelo nome.
    if (preferences.sort === "developer") {
      const da = a.developer?.trim(), db = b.developer?.trim();
      if (da !== db) { if (!da) return 1; if (!db) return -1; const delta = da.localeCompare(db, "pt-BR"); if (delta) return delta; }
    }
    return a.title.localeCompare(b.title, "pt-BR") * (preferences.descending ? -1 : 1);
  });
}
