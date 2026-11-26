import { invoke, isTauri } from "@tauri-apps/api/core";
import { DEFAULT_AVATAR, readMyPublicProfile } from "./publicProfile";

/**
 * Cartão do jogador para a tela final do Caravan no ShmupX Emulator: o avatar num círculo com o aro
 * ciano e o nick em Share Tech, num PNG transparente. O motor não desenha texto livre: o Client gera
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
    await document.fonts.load('72px "Share Tech"');

    const AVATAR = 144, RING = 6, GAP = 28, FONT = 72;
    const measure = document.createElement("canvas").getContext("2d")!;
    measure.font = `${FONT}px "Share Tech"`;
    const textWidth = Math.ceil(measure.measureText(name).width);
    const canvas = document.createElement("canvas");
    canvas.width = AVATAR + GAP + textWidth + 8;
    canvas.height = AVATAR;
    const ctx = canvas.getContext("2d")!;
    // Aro ciano e o avatar recortado em círculo.
    ctx.fillStyle = "#57fcff";
    ctx.beginPath();
    ctx.arc(AVATAR / 2, AVATAR / 2, AVATAR / 2, 0, Math.PI * 2);
    ctx.fill();
    ctx.save();
    ctx.beginPath();
    ctx.arc(AVATAR / 2, AVATAR / 2, AVATAR / 2 - RING, 0, Math.PI * 2);
    ctx.clip();
    ctx.fillStyle = "#101010";
    ctx.fillRect(0, 0, AVATAR, AVATAR);
    const side = Math.min(avatar.naturalWidth, avatar.naturalHeight);
    ctx.drawImage(avatar, (avatar.naturalWidth - side) / 2, (avatar.naturalHeight - side) / 2, side, side, RING, RING, AVATAR - RING * 2, AVATAR - RING * 2);
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
 * Campos de abertura do ShmupX Emulator para a partida Caravan: o tempo do Caravan do jogo e o
 * cartão do jogador da tela final. Fora do Caravan, nada.
 */
export async function caravanLaunchFields(seconds: number, caravan: boolean): Promise<{ caravanSegundos: number | null; cartaoJogador: boolean }> {
  if (!caravan) return { caravanSegundos: null, cartaoJogador: false };
  return { caravanSegundos: seconds, cartaoJogador: await preparePlayerCard("") };
}
