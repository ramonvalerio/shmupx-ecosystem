/**
 * Sessão do jogador em cada sala: o momento em que entrou. O chat só mostra as mensagens a partir
 * dele; sair da sala pela aba apaga a sessão (e a conversa reservada com o RobotX), então entrar
 * de novo começa com o chat limpo. Fechar o app sem sair mantém a sessão (o jogador continua na
 * sala). As mensagens continuam no servidor para quem ficou.
 */
const joinedKey = (uid: string, roomId: string) => `shmupx.client.roomJoinedAt.v1.${uid}.${roomId}`;
const robotxKey = (uid: string, roomId: string) => `shmupx.client.robotx.v1.${uid}.${roomId}`;

/** Momento da entrada na sala (ms); registra agora se ainda não havia. */
export function roomJoinedAt(uid: string, roomId: string): number {
  try {
    const saved = Number(localStorage.getItem(joinedKey(uid, roomId)));
    if (Number.isFinite(saved) && saved > 0) return saved;
    const now = Date.now();
    localStorage.setItem(joinedKey(uid, roomId), String(now));
    return now;
  } catch {
    return Date.now();
  }
}

/** Saiu da sala: a próxima entrada começa com o chat limpo. */
export function clearRoomSession(uid: string, roomId: string) {
  try {
    localStorage.removeItem(joinedKey(uid, roomId));
    localStorage.removeItem(robotxKey(uid, roomId));
  } catch { /* nada a limpar */ }
}
