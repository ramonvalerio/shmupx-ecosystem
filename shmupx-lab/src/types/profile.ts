/** Retângulo normalizado (0..1) relativo ao frame de referência. */
export interface Region {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type ProfileVariant = "lcd" | "crt";

/** Perfil de análise versionado publicado do Lab para o Client. */
export interface AnalysisProfile {
  gameId: string;
  version: string;
  variant: ProfileVariant;
  regions: {
    score: Region;
    lives?: Region;
    continues?: Region;
    gameOver?: Region;
  };
  /** Caminho/URL do modelo ONNX associado. */
  modelRef?: string;
}
