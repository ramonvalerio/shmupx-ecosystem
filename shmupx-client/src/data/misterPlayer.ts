/**
 * MiSTer ligado à conta neste PC: o IP, o jogador do `shmupx.ini` que é o dono da conta e a senha
 * do root, só se não for a padrão. Gravado ao conectar (Importar jogos › MiSTer FPGA) e usado pelo
 * INICIAR JOGO › MiSTer para abrir o jogo já com esse jogador.
 */
export interface MisterPlayer {
  host: string;
  username: string;
  password?: string;
}

const KEY = "shmupx.client.misterPlayer.v1";

export function readMisterPlayer(): MisterPlayer | null {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) ?? "null") as MisterPlayer | null;
    return value && typeof value.host === "string" && typeof value.username === "string" ? value : null;
  } catch {
    return null;
  }
}

export function saveMisterPlayer(value: MisterPlayer) {
  try { localStorage.setItem(KEY, JSON.stringify(value)); } catch { /* fica só nesta sessão */ }
}

/** Jogo no menu ShmupX do MiSTer (Rust `mister_jogos_do_jogador`). */
export interface MisterMenuGame {
  jogo: string;
  modo: string;
  caminho: string;
}
