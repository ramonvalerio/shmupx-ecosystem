import type { GameRoom } from "../types/room";
// Scripts de validação que o instalador leva (só os aprovados no Lab): o mesmo arquivo do build.
import scriptsAceitos from "../../src-tauri/scripts-aceitos.json";

/**
 * O Client tem o script de validação do jogo (a run é validada e entra no ranking). Sem ele, o jogo
 * aparece como Coming soon: dá para treinar, mas não jogar valendo ranking nem fazer live ranked.
 */
const SUPPORTED = new Set(scriptsAceitos.map((script) => script.game));
export const isSupported = (room: GameRoom) => SUPPORTED.has(room.id);
