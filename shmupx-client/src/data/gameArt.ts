import { useEffect, useState } from "react";
import type { GameRoom } from "../types/room";
import { useSteamLibrary } from "./steamLibrary";
import { useEmulatorLibrary } from "./emulatorLibrary";

/** Arte padrão do card: jogo sem imagem própria ("NO LOGO", a mesma do Client antigo). */
export const DEFAULT_COVER = "/img/no_logo.png";

const CURATED = "/img/games/curated";
const TITLES = "/img/games/emulator/titles";

/**
 * Artes que vieram do Client antigo (`public/img/games`), pelo nome do jogo normalizado (o id do
 * catálogo nem sempre é igual ao nome do arquivo). Jogos de arcade usam a tela de título da ROM.
 */
const ARTS: Record<string, string> = {
  "air-duel": `${CURATED}/air-duel.png`,
  "battle-garegga": `${CURATED}/battle-garegga.png`,
  "dodonpachi": `${CURATED}/dodonpachi.png`,
  "dodonpachi-resurrection": `${CURATED}/dodonpachi.png`,
  "earthion": `${CURATED}/earthion.png`,
  "espgaluda": `${CURATED}/espgaluda.png`,
  "grind-stormer": `${CURATED}/grind-stormer.png`,
  "ketsui": `${CURATED}/ketsui-kizuna-jigoku-tachi.png`,
  "ketsui-kizuna-jigoku-tachi": `${CURATED}/ketsui-kizuna-jigoku-tachi.png`,
  "aero-fighters-2": `${TITLES}/sonicwi2.png`,
  "sonic-wings-2": `${TITLES}/sonicwi2.png`,
  "darius-gaiden": `${TITLES}/dariusg.png`,
  "air-buster": `${TITLES}/airbustr.png`,
  "akatana": `${TITLES}/akatana.png`,
  "batsugun": `${TITLES}/batsugun.png`,
  "blazing-star": `${TITLES}/blazstar.png`,
  "carrier-air-wing": `${TITLES}/cawing.png`,
  "dodonpachi-daioujou": `${TITLES}/ddp3.png`,
  "espgaluda-ii": `${TITLES}/espgal2.png`,
  "esprade": `${TITLES}/esprade.png`,
  "gradius-iii": `${TITLES}/gradius3.png`,
  "gunbird": `${TITLES}/gunbird.png`,
  "gunbird-2": `${TITLES}/gunbird2.png`,
  "guwange": `${TITLES}/guwange.png`,
  "mazinger-z": `${TITLES}/mazinger.png`,
  "raiden": `${TITLES}/raiden.png`,
  "raiden-ii": `${TITLES}/raiden2.png`,
  "raiden-dx": `${TITLES}/raidendx.png`,
  "samurai-aces": `${TITLES}/samuraia.png`,
  "sengoku-ace": `${TITLES}/samuraia.png`,
  "truxton": `${TITLES}/truxton.png`,
  "truxton-ii": `${TITLES}/truxton2.png`,
  "zed-blade": `${TITLES}/zedblade.png`,
};

const slug = (text: string) => text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

/** Arte do Client antigo para o jogo, pelo id ou pelo título. */
export function legacyArt(room: GameRoom): string | undefined {
  return ARTS[room.id] ?? ARTS[slug(room.title)];
}

/**
 * Arte do jogo, em ordem: a capa enviada pelo Lab, a arte do Client antigo e a arte padrão do
 * ShmupX. Se uma falha ao carregar, passa para a próxima.
 *
 * `grayscale`: o jogo não foi importado pelo jogador (não está na biblioteca da Steam nem tem o ROM
 * na biblioteca do emulador), como no Client antigo.
 */
export function useGameArt(room: GameRoom) {
  const { owns } = useSteamLibrary();
  const emulator = useEmulatorLibrary();
  const candidates = [room.coverUrl, legacyArt(room), DEFAULT_COVER].filter((src): src is string => Boolean(src));
  const [index, setIndex] = useState(0);
  useEffect(() => setIndex(0), [room.id, room.coverUrl]);
  const src = candidates[Math.min(index, candidates.length - 1)];
  return {
    src,
    kind: src === DEFAULT_COVER ? "is-default" : src === room.coverUrl ? "has-cover" : "is-curated",
    // O "NO LOGO" é um aviso, não a arte do jogo: fica sempre colorido (como no Client antigo).
    grayscale: src !== DEFAULT_COVER && !((room.steamAppId > 0 && owns(room.steamAppId)) || emulator.owns(room)),
    onError: () => setIndex((current) => Math.min(current + 1, candidates.length - 1)),
  };
}
