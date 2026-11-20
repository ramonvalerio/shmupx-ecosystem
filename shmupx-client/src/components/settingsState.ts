export type Appearance = "default" | "dark" | "light";
export type StartupSection = "home" | "collection" | "ranking" | "settings";
export interface ClientPreferences { appearance: Appearance; compact: boolean; startup: StartupSection; startupBigPicture: boolean; language: "pt-BR" | "en"; }
export const PREFERENCES_KEY = "shmupx.client.appearance.v1";
export function defaultPreferences(): ClientPreferences { return { appearance: "default", compact: false, startup: "collection", startupBigPicture: false, language: "pt-BR" }; }
export function validatePreferences(value: unknown): ClientPreferences {
  if (!value || typeof value !== "object") return defaultPreferences();
  const saved = value as Partial<ClientPreferences>;
  return {
    appearance: saved.appearance === "dark" || saved.appearance === "light" ? saved.appearance : "default",
    compact: saved.compact === true,
    // A Coleção é a tela principal: é o padrão quando nada (válido) foi escolhido.
    startup: saved.startup === "home" || saved.startup === "ranking" || saved.startup === "settings" ? saved.startup : "collection",
    startupBigPicture: saved.startupBigPicture === true,
    language: saved.language === "en" ? "en" : "pt-BR",
  };
}
export function readPreferences(): ClientPreferences {
  try { return validatePreferences(JSON.parse(localStorage.getItem(PREFERENCES_KEY) ?? "null")); }
  catch { return defaultPreferences(); }
}
