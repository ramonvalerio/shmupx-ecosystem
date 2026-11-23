import { useCallback, useEffect, useState } from "react";
import type { GameMode } from "@client/types/room";
import { PREFERENCES_KEY } from "@client/components/settingsState";
import type { Language } from "@client/i18nMessages";

/** Onde o celular abre o jogo: Steam e Emulador no PC (Client na mesma conta), MiSTer na TV. */
export type Platform = "steam" | "emulator" | "mister";
export type Kind = "training" | "ranked";

/** Pré-configuração da tela inicial: o ▶ da lista usa ela, e a folha de Play da sala vem com ela marcada. */
export interface PlayPreset { platform: Platform; mode: GameMode; kind: Kind }

const PRESET_KEY = "shmupx.lite.play.v1";
const THEME_KEY = "shmupx.lite.theme.v1";
const DEFAULT_PRESET: PlayPreset = { platform: "mister", mode: "highscore", kind: "training" };

function readJson(key: string): unknown {
  try { return JSON.parse(localStorage.getItem(key) ?? "null"); } catch { return null; }
}
function writeJson(key: string, value: unknown) {
  try { localStorage.setItem(key, JSON.stringify(value)); } catch { /* fica só nesta sessão */ }
}

export function readPreset(): PlayPreset {
  const saved = readJson(PRESET_KEY) as Partial<PlayPreset> | null;
  return {
    platform: saved?.platform === "steam" || saved?.platform === "emulator" || saved?.platform === "mister" ? saved.platform : DEFAULT_PRESET.platform,
    mode: saved?.mode === "highscore" || saved?.mode === "1cc" || saved?.mode === "caravan" ? saved.mode : DEFAULT_PRESET.mode,
    kind: saved?.kind === "ranked" ? "ranked" : "training",
  };
}

/** Pré-configuração salva no aparelho; todas as telas leem a mesma. */
export function usePreset(): [PlayPreset, (next: PlayPreset) => void] {
  const [preset, setPreset] = useState(readPreset);
  const save = useCallback((next: PlayPreset) => { setPreset(next); writeJson(PRESET_KEY, next); }, []);
  return [preset, save];
}

/** Temas do Lite: o padrão (cinza escuro e ciano), preto (fundo preto puro) e branco. */
export type Theme = "default" | "black" | "white";
export const THEMES: { id: Theme; label: string }[] = [
  { id: "default", label: "Padrão" },
  { id: "black", label: "Preto" },
  { id: "white", label: "Branco" },
];

export function readTheme(): Theme {
  const saved = readJson(THEME_KEY);
  return saved === "black" || saved === "white" ? saved : "default";
}

export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
  document.documentElement.style.colorScheme = theme === "white" ? "light" : "dark";
}

export function useTheme(): [Theme, (next: Theme) => void] {
  const [theme, setTheme] = useState(readTheme);
  useEffect(() => applyTheme(theme), [theme]);
  const save = useCallback((next: Theme) => { setTheme(next); writeJson(THEME_KEY, next); }, []);
  return [theme, save];
}

/** Idioma: o mesmo registro de preferências do Client (o I18nProvider lê dele ao abrir). */
export function saveLanguage(language: Language) {
  const current = readJson(PREFERENCES_KEY);
  writeJson(PREFERENCES_KEY, { ...(current && typeof current === "object" ? current : {}), language });
}
