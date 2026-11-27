import { useSyncExternalStore } from "react";

/**
 * Qualidade da conexão entre dois jogadores da sala (as barrinhas na lista de jogadores), sem
 * expor o IP de ninguém: cada Client mede o próprio ping até alguns pontos de referência da AWS
 * (São Paulo, EUA, Europa e Japão) e publica só esses números na presença da sala. O ping entre
 * dois jogadores é estimado pela melhor rota entre eles: o menor (meu ping + ping do outro) até um
 * mesmo ponto. É um teto (o caminho direto costuma ser um pouco mais curto), bom para comparar.
 */
const REGIONS = {
  sa: "sa-east-1",
  use: "us-east-1",
  usw: "us-west-1",
  euw: "eu-west-1",
  euc: "eu-central-1",
  apne: "ap-northeast-1",
} as const;
export type NetRegion = keyof typeof REGIONS;
/** Ping (ms) até cada ponto de referência. */
export type NetProfile = Partial<Record<NetRegion, number>>;
const REGION_KEYS = Object.keys(REGIONS) as NetRegion[];

/** Medir de novo de tempos em tempos (a rede muda: Wi-Fi, 4G, VPN). */
const MEDIR_A_CADA_MS = 15 * 60_000;
const TIMEOUT_MS = 3_000;
const MAX_PING = 3_000;

let profile: NetProfile | null = null;
let measuredAt = 0;
let measuring = false;
const listeners = new Set<() => void>();

async function pingRegion(region: string): Promise<number | null> {
  const url = `https://dynamodb.${region}.amazonaws.com/ping`;
  const once = async (n: number) => {
    const controller = new AbortController();
    const timer = window.setTimeout(() => controller.abort(), TIMEOUT_MS);
    try {
      const started = performance.now();
      // sem CORS no ponto de referência: a resposta vem opaca, só o tempo interessa
      await fetch(`${url}?n=${n}-${Date.now()}`, { mode: "no-cors", cache: "no-store", signal: controller.signal });
      return performance.now() - started;
    } finally {
      window.clearTimeout(timer);
    }
  };
  try {
    await once(0); // a primeira abre a conexão (DNS e TLS) e não conta
    let best = Infinity;
    for (let n = 1; n <= 3; n++) best = Math.min(best, await once(n));
    return Math.min(MAX_PING, Math.max(1, Math.round(best)));
  } catch {
    return null;
  }
}

/** Mede de novo se a última medição for antiga (não mede com jogo, gravação ou live: `busy`). */
export function refreshNetProfile(busy: boolean) {
  if (busy || measuring || Date.now() - measuredAt < MEDIR_A_CADA_MS) return;
  measuring = true;
  void Promise.all(REGION_KEYS.map(async (key) => [key, await pingRegion(REGIONS[key])] as const))
    .then((results) => {
      const next: NetProfile = {};
      for (const [key, ms] of results) if (ms !== null) next[key] = ms;
      profile = Object.keys(next).length > 0 ? next : null;
      measuredAt = Date.now();
      listeners.forEach((listener) => listener());
    })
    .finally(() => { measuring = false; });
}

/** A medição atual deste PC (null antes da primeira ou sem rede). */
export function currentNetProfile(): NetProfile | null {
  return profile;
}

export function useNetProfile(): NetProfile | null {
  return useSyncExternalStore((listener) => { listeners.add(listener); return () => listeners.delete(listener); }, () => profile);
}

/** Lê o perfil publicado na presença (ignora o que não for um ping válido). */
export function netProfileFrom(value: unknown): NetProfile | undefined {
  if (!value || typeof value !== "object") return undefined;
  const out: NetProfile = {};
  for (const key of REGION_KEYS) {
    const ms = (value as Record<string, unknown>)[key];
    if (typeof ms === "number" && Number.isFinite(ms) && ms >= 1 && ms <= MAX_PING) out[key] = ms;
  }
  return Object.keys(out).length > 0 ? out : undefined;
}

/** Ping estimado entre dois jogadores (ms), ou null sem medição em comum. */
export function estimatePing(a: NetProfile | null | undefined, b: NetProfile | null | undefined): number | null {
  if (!a || !b) return null;
  let best: number | null = null;
  for (const key of REGION_KEYS) {
    const x = a[key];
    const y = b[key];
    if (x !== undefined && y !== undefined && (best === null || x + y < best)) best = x + y;
  }
  return best;
}

export type ConnectionLevel = "good" | "fair" | "poor";
/** Verde até 80 ms, amarelo até 150 ms e vermelho acima (no rollback da dupla, acima de ~150 ms
 *  os acertos de quadro já aparecem). */
export function connectionLevel(ping: number): ConnectionLevel {
  return ping <= 80 ? "good" : ping <= 150 ? "fair" : "poor";
}
