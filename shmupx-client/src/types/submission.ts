/** Runs observed by this client session; no synthetic scores or submission confirmations. */
export interface SessionRun {
  id: string;
  gameId: string;
  startedAt: number;
  endedAt?: number;
  nativePid?: number;
  /** Id da run no servidor (chega com a análise; runs "Não enviada" guardadas já têm). */
  runId?: string;
  /** Placar enviado ao servidor. */
  score?: number;
  message?: string;
  status: "recording" | "analyzing" | "submitted" | "expired" | "failed";
  /** Posição da run no ranking do jogo (quando é o melhor placar do jogador). */
  position?: number;
  /** Pódio (top 3 do Hi-Score) do jogo logo depois desta run: a sala reordena se mudou. */
  podium?: PodiumPlace[];
  /** Ranked com live no top 3: a live indo para o canal oficial da ShmupX. */
  highlight?: RunHighlight;
}

/** Um dos 3 primeiros do ranking do jogo da sala. */
export interface PodiumPlace {
  posicao: number;
  jogador_id: string;
  nome: string;
  score: number;
}

export interface RunHighlight {
  position: number;
  status: "uploading" | "posted" | "failed";
  /** 0 a 1, enquanto envia. */
  progress?: number;
  videoUrl?: string;
}

/** Andamento do destaque (Rust `live::destaque`, evento `run://destaque`). */
export interface RunHighlightEvent {
  jogo_id: string;
  run_id: string;
  posicao: number;
  status: RunHighlight["status"];
  video_url?: string | null;
  progresso?: number | null;
}

export interface RunAnalysisEvent {
  jogo_id: string;
  pid: number;
  run_id?: string | null;
  status: "analyzing" | "submitted" | "failed" | "discarded";
  message?: string | null;
  score?: number | null;
  position?: number | null;
  podium?: PodiumPlace[] | null;
}

/** Run "Não enviada" guardada pelo Client (`runs_pendentes`), com o vídeo ainda no disco. */
export interface PendingRun {
  run_id: string;
  jogo_id: string;
  iniciada_em: number;
  encerrada_em?: number | null;
  erro: string;
}

export function pendingToSessionRun(pending: PendingRun): SessionRun {
  return {
    id: `pending-${pending.run_id}`, runId: pending.run_id, gameId: pending.jogo_id,
    startedAt: pending.iniciada_em, endedAt: pending.encerrada_em ?? undefined, status: "failed", message: pending.erro,
  };
}

/** Junta as runs guardadas às desta sessão, sem repetir a mesma run do servidor. */
export function mergePendingRuns(runs: SessionRun[], pending: PendingRun[]): SessionRun[] {
  const known = new Set(runs.map((run) => run.runId).filter(Boolean));
  const extra = pending.filter((item) => !known.has(item.run_id)).map(pendingToSessionRun);
  return extra.length ? [...runs, ...extra] : runs;
}

export function applyRunAnalysis(runs: SessionRun[], event: RunAnalysisEvent, now: number): SessionRun[] {
  if (!["analyzing", "submitted", "failed", "discarded"].includes(event.status)) return runs;
  const runId = event.run_id ?? undefined;
  if (event.status === "discarded") return runId ? runs.filter((run) => run.runId !== runId) : runs;
  return runs.map((run) => {
    // "Analisar de novo" acha a run pelo id do servidor (pode estar "Não enviada").
    const sameRun = runId !== undefined && run.runId === runId;
    const samePid = run.nativePid !== undefined && run.nativePid === event.pid && run.gameId === event.jogo_id &&
      run.status !== "submitted" && run.status !== "failed";
    if (!sameRun && !samePid) return run;
    return { ...run, runId: runId ?? run.runId, status: event.status as SessionRun["status"], endedAt: run.endedAt ?? now,
      message: event.message ?? undefined, score: event.score ?? run.score,
      position: event.position ?? run.position, podium: event.podium ?? run.podium };
  });
}

export function applyRunHighlight(runs: SessionRun[], event: RunHighlightEvent): SessionRun[] {
  return runs.map((run) => run.runId !== event.run_id ? run : {
    ...run,
    highlight: { position: event.posicao, status: event.status, progress: event.progresso ?? undefined, videoUrl: event.video_url ?? undefined },
  });
}

/** Mesmo pódio (mesmos jogadores nas mesmas posições)? */
export function samePodium(a: ReadonlyMap<string, number>, b: ReadonlyMap<string, number>): boolean {
  return a.size === b.size && [...a].every(([player, place]) => b.get(player) === place);
}

/** Pódio da sala: jogador → posição (1º a 3º). */
export function podiumPlaces(places: readonly { posicao: number; jogador_id: string }[]): Map<string, number> {
  const map = new Map<string, number>();
  for (const place of places) if (place.posicao <= 3) map.set(place.jogador_id, place.posicao);
  return map;
}
