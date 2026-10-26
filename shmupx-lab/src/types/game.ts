/** Jogo cadastrado no Lab (mock por ora — ver TASKS.md 1.1 para persistência real em SQLite). */
export interface LabGame {
  id: string;
  title: string;
  steamAppId: number;
  /** Versão semver do perfil de análise atualmente publicado para este jogo. */
  profileVersion: string;
}
