/** As três regiões obrigatórias do Spec 001 para o primeiro ciclo de mapeamento. */
export type MappingRegionKey = "score" | "lives" | "gameOver";

/**
 * Bounding box em pixels exatos sobre a imagem de referência carregada (não normalizado).
 * A conversão para coordenadas 0..1 (`Region` em `types/profile.ts`) acontece na exportação
 * do perfil, que ainda depende das dimensões naturais da imagem usada como referência.
 */
export interface MappingBox {
  key: MappingRegionKey;
  label: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

export const REQUIRED_MAPPING_LABELS: Record<MappingRegionKey, string> = {
  score: "High Score",
  lives: "Vidas Restantes",
  gameOver: "Tela de Game Over",
};

/** Posições/tamanhos iniciais plausíveis para uma referência ~1920×1080; o admin ajusta por drag. */
export function createDefaultMappingBoxes(): MappingBox[] {
  return [
    { key: "score", label: REQUIRED_MAPPING_LABELS.score, x: 40, y: 40, width: 260, height: 70 },
    { key: "lives", label: REQUIRED_MAPPING_LABELS.lives, x: 40, y: 130, width: 260, height: 70 },
    {
      key: "gameOver",
      label: REQUIRED_MAPPING_LABELS.gameOver,
      x: 700,
      y: 420,
      width: 520,
      height: 160,
    },
  ];
}
