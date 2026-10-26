export type Appearance = "default" | "dark" | "light";
export type StartupSection = "home" | "collection" | "ranking" | "settings";
export interface ClientPreferences { appearance: Appearance; compact: boolean; startup: StartupSection; startupBigPicture: boolean; language: "pt-BR" | "en"; }
export const PREFERENCES_KEY = "shmupx.client.appearance.v1";
export function defaultPreferences(): ClientPreferences { return { appearance: "default", compact: false, startup: "home", startupBigPicture: false, language: "pt-BR" }; }
export function validatePreferences(value: unknown): ClientPreferences {
  if (!value || typeof value !== "object") return defaultPreferences();
  const saved = value as Partial<ClientPreferences>;
  return {
    appearance: saved.appearance === "dark" || saved.appearance === "light" ? saved.appearance : "default",
    compact: saved.compact === true,
    startup: saved.startup === "collection" || saved.startup === "ranking" || saved.startup === "settings" ? saved.startup : "home",
    startupBigPicture: saved.startupBigPicture === true,
    language: saved.language === "en" ? "en" : "pt-BR",
  };
}
export function readPreferences(): ClientPreferences {
  try { return validatePreferences(JSON.parse(localStorage.getItem(PREFERENCES_KEY) ?? "null")); }
  catch { return defaultPreferences(); }
}
