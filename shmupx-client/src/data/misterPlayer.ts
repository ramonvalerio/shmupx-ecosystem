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

/**
 * Usuário e senha do MiSTer (SSH e FTP), só quando não são os de fábrica (`root` / `1`): vazio usa
 * o padrão e conecta direto.
 */
export interface MisterAccess {
  user?: string;
  password?: string;
}

const ACCESS_KEY = "shmupx.client.misterAccess.v1";

export function readMisterAccess(): MisterAccess {
  try {
    const value = JSON.parse(localStorage.getItem(ACCESS_KEY) ?? "{}") as MisterAccess;
    return { user: typeof value.user === "string" && value.user.trim() ? value.user.trim() : undefined, password: typeof value.password === "string" && value.password ? value.password : undefined };
  } catch {
    return {};
  }
}

export function saveMisterAccess(value: MisterAccess) {
  try {
    if (value.user?.trim() || value.password) localStorage.setItem(ACCESS_KEY, JSON.stringify({ user: value.user?.trim() || undefined, password: value.password || undefined }));
    else localStorage.removeItem(ACCESS_KEY);
  } catch { /* fica só nesta sessão */ }
}

/** `usuario` e `senha` dos comandos do MiSTer (nulos = `root` / `1`). `password`: a informada na tela. */
export function misterAuth(password?: string | null): { usuario: string | null; senha: string | null } {
  const access = readMisterAccess();
  return { usuario: access.user ?? null, senha: password || access.password || null };
}

/** Jogo no menu ShmupX do MiSTer (Rust `mister_jogos_do_jogador`). */
export interface MisterMenuGame {
  jogo: string;
  modo: string;
  caminho: string;
}
