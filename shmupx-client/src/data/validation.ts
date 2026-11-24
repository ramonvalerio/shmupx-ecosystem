import type { GameRoom } from "../types/room";
// Scripts de validação que o instalador leva (só os aprovados no Lab): o mesmo arquivo do build.
import scriptsAceitos from "../../src-tauri/scripts-aceitos.json";
// Jogos validados pela memória no ShmupX Emulator (placar lido do jogo, sem script de vídeo).
import jogosPelaMemoria from "../../src-tauri/jogos-pela-memoria.json";

/**
 * O Client valida as runs do jogo (pelo script de vídeo ou pela memória no ShmupX Emulator) e elas
 * entram no ranking. Sem nenhum dos dois, o jogo aparece como Coming soon: dá para treinar, mas não
 * jogar valendo ranking nem fazer live ranked.
 */
const SUPPORTED = new Set([...scriptsAceitos, ...jogosPelaMemoria].map((item) => item.game));
export const isSupported = (room: GameRoom) => SUPPORTED.has(room.id);
