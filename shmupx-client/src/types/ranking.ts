/** Linha de `GET /api/ranking/jogos/{jogo_id}` (shmupx-backend, routes/ranking_publico.rs). */
export interface GameRankingEntry {
  posicao: number;
  jogador_id: string;
  nome: string;
  score: number;
  nota: number;
  continues: number;
  vidas_perdidas: number | null;
  runs: number;
  modo: string | null;
  versao: string | null;
  origem: string | null;
  /** `false` enquanto a gameplay está em revisão (`needs_review`). */
  verificada: boolean;
  conquistado_em: string | null;
  // Ainda não enviados por routes/ranking_publico.rs: a tela de ranking só mostra quando vierem.
  /** Onde a run foi jogada: "steam" | "emulator" | "mister". */
  plataforma?: string | null;
  /** Início da gameplay (ISO), `capture.serverStartedAt`. */
  iniciada_em?: string | null;
  duracao_segundos?: number | null;
  /** Link do vídeo da run no YouTube (`https://www.youtube.com/watch?v=…`). */
  youtube_url?: string | null;
}

export interface GameRanking {
  jogo_id: string;
  titulo: string;
  modo: string | null;
  versao: string | null;
  total: number;
  truncado: boolean;
  atualizado_em: string;
  entradas: GameRankingEntry[];
}

/** Linha de `GET /api/ranking/global`: soma das notas dos 5 jogos mais jogados (Spec 002). */
export interface GlobalRankingEntry {
  posicao: number;
  jogador_id: string;
  nome: string;
  pontos: number;
  jogos_contados: number;
  runs: number;
  jogos: { jogo_id: string; titulo: string; nota: number; runs: number }[];
}

export interface GlobalRanking {
  total: number;
  truncado: boolean;
  atualizado_em: string;
  entradas: GlobalRankingEntry[];
}
