/** Runs observed by this client session; no synthetic scores or submission confirmations. */
export interface SessionRun {
  id: string;
  gameId: string;
  startedAt: number;
  endedAt?: number;
  nativePid?: number;
  message?: string;
  status: "recording" | "analyzing" | "submitted" | "expired" | "failed";
}

export interface RunAnalysisEvent {
  jogo_id: string;
  pid: number;
  status: "analyzing" | "submitted" | "failed";
  message?: string | null;
}

export function applyRunAnalysis(runs: SessionRun[], event: RunAnalysisEvent, now: number): SessionRun[] {
  if (!["analyzing", "submitted", "failed"].includes(event.status)) return runs;
  return runs.map((run) => {
    if (run.nativePid !== event.pid || run.gameId !== event.jogo_id ||
        run.status === "submitted" || run.status === "failed") return run;
    return { ...run, status: event.status, endedAt: run.endedAt ?? now,
      message: event.message ?? undefined };
  });
}
