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
}

export interface RunAnalysisEvent {
  jogo_id: string;
  pid: number;
  run_id?: string | null;
  status: "analyzing" | "submitted" | "failed" | "discarded";
  message?: string | null;
  score?: number | null;
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
      message: event.message ?? undefined, score: event.score ?? run.score };
  });
}
