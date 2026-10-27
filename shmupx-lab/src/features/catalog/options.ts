/** Mesmos valores que o backend aceita em `games/{id}` (shmupx-backend, `services/catalogo.rs`). */
export const PLATFORMS = [
  { value: "steam", label: "Steam" },
  { value: "megadrive", label: "Mega Drive" },
  { value: "neogeo", label: "Neo Geo" },
  { value: "arcade", label: "Arcade" },
  { value: "xbox360", label: "Xbox 360" },
  { value: "playstation", label: "PlayStation" },
  { value: "switch", label: "Nintendo Switch" },
  { value: "other", label: "Outra" },
] as const;

/** Formatos competitivos que o jogo suporta. */
export const MODES = [
  { value: "highscore", label: "High score" },
  { value: "1cc", label: "1CC (1 crédito)" },
  { value: "caravan", label: "Caravan" },
] as const;

export function platformLabel(value: string | undefined): string {
  return PLATFORMS.find((platform) => platform.value === value)?.label ?? "";
}

/** ID curto sugerido a partir do nome: minúsculas, sem acento, hífens (ex.: "DoDonPachi" -> "dodonpachi"). */
export function slugFromName(name: string): string {
  return name
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 40);
}
