import { invoke, isTauri } from "@tauri-apps/api/core";
import { DEFAULT_AVATAR, readMyPublicProfile } from "./publicProfile";

/**
 * Cartão do jogador para a tela final do Caravan no ShmupX Emulator: a foto do avatar em círculo, sem
 * borda, e o nick em Share Tech, num PNG transparente. O motor não desenha texto livre: o Client gera
 * a imagem e o plugin do ShmupX a mostra acima dos pontos e das vidas perdidas. Devolve `true`
 * quando o cartão ficou salvo (o Rust passa o caminho ao motor).
 */
export async function preparePlayerCard(fallbackName: string): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    const mine = readMyPublicProfile();
    const name = (mine?.handle || fallbackName || "").trim();
    if (!name) return false;
    const avatar = await loadImage(await avatarSource(mine?.avatar || DEFAULT_AVATAR));
    await document.fonts.load('84px "Share Tech"');

    const AVATAR = 176, GAP = 30, FONT = 84;
    const measure = document.createElement("canvas").getContext("2d")!;
    measure.font = `${FONT}px "Share Tech"`;
    const textWidth = Math.ceil(measure.measureText(name).width);
    const canvas = document.createElement("canvas");
    canvas.width = AVATAR + GAP + textWidth + 8;
    canvas.height = AVATAR;
    const ctx = canvas.getContext("2d")!;
    // A foto pura, recortada em círculo, sem aro nem borda.
    ctx.save();
    ctx.beginPath();
    ctx.arc(AVATAR / 2, AVATAR / 2, AVATAR / 2, 0, Math.PI * 2);
    ctx.clip();
    const side = Math.min(avatar.naturalWidth, avatar.naturalHeight);
    ctx.drawImage(avatar, (avatar.naturalWidth - side) / 2, (avatar.naturalHeight - side) / 2, side, side, 0, 0, AVATAR, AVATAR);
    ctx.restore();
    // Nick em branco com contorno escuro, como os números do contador.
    ctx.font = `${FONT}px "Share Tech"`;
    ctx.textBaseline = "middle";
    ctx.lineJoin = "round";
    ctx.lineWidth = 8;
    ctx.strokeStyle = "#100e2c";
    ctx.strokeText(name, AVATAR + GAP, AVATAR / 2 + 2);
    ctx.fillStyle = "#f0fcff";
    ctx.fillText(name, AVATAR + GAP, AVATAR / 2 + 2);

    const png = canvas.toDataURL("image/png").split(",")[1];
    await invoke("salvar_cartao_jogador", { pngBase64: png });
    return true;
  } catch (error) {
    console.warn("cartão do jogador do Caravan não gerado:", error);
    return false;
  }
}

/** Avatar de outro site (Google etc.): o Rust baixa e devolve como data: URL, para o canvas poder exportar. */
async function avatarSource(url: string): Promise<string> {
  return /^https?:\/\//.test(url) ? invoke<string>("imagem_como_data_url", { url }) : url;
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error(`avatar não carregou: ${src.slice(0, 60)}`));
    image.src = src;
  });
}

/**
 * Campos de abertura do ShmupX Emulator para uma partida ranked: o tempo do Caravan (só no Caravan) e
 * o cartão do jogador da tela final (game over ou fim do tempo). Em treino, nada.
 */
export async function rankedLaunchFields(seconds: number, caravan: boolean, ranked: boolean): Promise<{ caravanSegundos: number | null; cartaoJogador: boolean }> {
  if (!ranked) return { caravanSegundos: null, cartaoJogador: false };
  return { caravanSegundos: caravan ? seconds : null, cartaoJogador: await preparePlayerCard("") };
}

/**
 * Imagem da posição no ranking para a tela final do motor: "RANK" em azul (rótulo) e o número em
 * branco (valor), com a medalha (ouro, prata ou bronze) ao lado no top 3. Chega depois que o servidor aceita a run; o motor a mostra assim
 * que o arquivo aparece. Fora do app desktop, nada.
 */
export async function publishPlacement(position: number): Promise<void> {
  if (!isTauri() || !Number.isInteger(position) || position < 1) return;
  try {
    await document.fonts.load('64px "Share Tech"');
    const HEIGHT = 112, GAP = 22, CYAN = "#57fcff", WHITE = "#ffffff";
    const medal = position <= 3 ? await loadImage(`/img/medals/medal-${position}.png`) : null;
    const label = "RANK ", value = String(position);
    const measure = document.createElement("canvas").getContext("2d")!;
    measure.font = `${HEIGHT * 0.66}px "Share Tech"`;
    const labelWidth = measure.measureText(label).width;
    const textWidth = Math.ceil(labelWidth + measure.measureText(value).width);
    const canvas = document.createElement("canvas");
    canvas.width = (medal ? HEIGHT + GAP : 0) + textWidth + 8;
    canvas.height = HEIGHT;
    const ctx = canvas.getContext("2d")!;
    let x = 0;
    if (medal) { ctx.drawImage(medal, 0, 0, HEIGHT, HEIGHT); x = HEIGHT + GAP; }
    ctx.font = `${HEIGHT * 0.66}px "Share Tech"`;
    ctx.textBaseline = "middle";
    ctx.lineJoin = "round";
    ctx.lineWidth = 8;
    ctx.strokeStyle = "#100e2c";
    // Rótulo em azul e valor em branco, os dois com o contorno escuro dos números do motor.
    ctx.strokeText(label, x, HEIGHT / 2 + 3);
    ctx.fillStyle = CYAN;
    ctx.fillText(label, x, HEIGHT / 2 + 3);
    ctx.strokeText(value, x + labelWidth, HEIGHT / 2 + 3);
    ctx.fillStyle = WHITE;
    ctx.fillText(value, x + labelWidth, HEIGHT / 2 + 3);
    await invoke("salvar_posicao_jogador", { pngBase64: canvas.toDataURL("image/png").split(",")[1] });
  } catch (error) {
    console.warn("posição do jogador não gerada:", error);
  }
}
