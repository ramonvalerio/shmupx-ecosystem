/** Mesmos valores que o backend aceita em `games/{id}` (shmupx-backend, `services/catalogo.rs`). */
export const PLATFORMS = [
  { value: "steam", label: "Steam" },
  { value: "mister", label: "MiSTer FPGA" },
  { value: "megadrive", label: "Mega Drive" },
  { value: "neogeo", label: "Neo Geo" },
  { value: "arcade", label: "Arcade" },
  { value: "xbox360", label: "Xbox 360" },
  { value: "playstation", label: "PlayStation" },
  { value: "switch", label: "Nintendo Switch" },
  { value: "other", label: "Outra" },
] as const;

/** Onde o jogo pode ser jogado no ShmupX (mais de uma). Mesmos valores do backend. */
export const PLAY_ON = [
  { value: "steam", label: "Steam" },
  { value: "emulator", label: "Emulador" },
  { value: "mister", label: "MiSTer" },
] as const;

/** Plataformas de um jogo salvo antes da escolha múltipla: pela plataforma antiga e pelo app_id. */
export function playOnOf(game: { platforms?: string[]; platform?: string; steam_app_id: number }): string[] {
  if (game.platforms?.length) return game.platforms;
  const result = new Set<string>();
  if (game.steam_app_id > 0 || game.platform === "steam") result.add("steam");
  if (game.platform === "mister") result.add("mister");
  if (game.platform === "arcade" || game.platform === "neogeo" || game.platform === "megadrive") result.add("emulator");
  return [...result];
}

export function playOnLabels(values: string[]): string {
  return PLAY_ON.filter((p) => values.includes(p.value)).map((p) => p.label).join(", ");
}

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

const DATA_HORA = new Intl.DateTimeFormat("pt-BR", { day: "2-digit", month: "2-digit", year: "numeric", hour: "2-digit", minute: "2-digit" });

/** Data e hora locais curtas (ex.: 24/11/2026, 14:30); `—` quando o backend não informou. */
export function formatDateTime(iso: string | undefined): string {
  if (!iso) return "—";
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? "—" : DATA_HORA.format(date);
}
