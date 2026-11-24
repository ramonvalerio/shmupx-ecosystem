import type { GameRoom } from "../types/room";

/**
 * ROMs de cada jogo do catálogo (nomes curtos do MAME/FBNeo). O ROM é ligado ao jogo pelo nome (id ou
 * título normalizado) com esta tabela, a mesma do Client antigo; sem entrada na tabela, vale o
 * título sem separadores (`R-Type` → `rtype`), o nome curto que o FBNeo usa em muitos jogos.
 */
const ROM_GAMES: Record<string, string[]> = {
  ket: ["ketsui", "ketsui-kizuna-jigoku-tachi"],
  esprade: ["esprade", "esp-ra-de"],
  espgal: ["espgaluda"],
  espgal2: ["espgaluda-ii", "espgaluda-2"],
  guwange: ["guwange"],
  bgaregga: ["battle-garegga"],
  airduel: ["air-duel"],
  dariusg: ["darius-gaiden"],
  dariusgx: ["darius-gaiden"],
  ddonpach: ["dodonpachi"],
  ddp3: ["dodonpachi-daioujou", "dodonpachi-dai-ou-jou"],
  ddpdoj: ["dodonpachi-daioujou", "dodonpachi-dai-ou-jou"],
  ddpdojblk: ["dodonpachi-daioujou-black-label", "dodonpachi-dai-ou-jou-black-label"],
  gunbird: ["gunbird"],
  gunbird2: ["gunbird-2"],
  blazstar: ["blazing-star"],
  sonicwi2: ["aero-fighters-2", "sonic-wings-2"],
  aerofgt: ["aero-fighters", "sonic-wings"],
  sonicwi: ["aero-fighters", "sonic-wings"],
  samuraia: ["samurai-aces", "sengoku-ace"],
  truxton: ["truxton", "tatsujin"],
  truxton2: ["truxton-ii", "truxton-2", "tatsujin-oh"],
  zedblade: ["zed-blade"],
  batsugun: ["batsugun"],
  batrider: ["armed-police-batrider", "batrider"],
  grindstm: ["grind-stormer"],
  gradius3: ["gradius-iii", "gradius-3"],
  raiden: ["raiden"],
  raiden2: ["raiden-ii", "raiden-2"],
  raidendx: ["raiden-dx"],
  mazinger: ["mazinger-z"],
  akatana: ["akai-katana"],
  cawing: ["carrier-air-wing"],
  airbustr: ["air-buster"],
  rtype: ["r-type"],
  rtype2: ["r-type-ii", "r-type-2"],
  rtypeleo: ["r-type-leo"],
};

const slug = (text: string) => text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

/** ROMs da tabela para um jogo do catálogo (vazio = o Client não conhece o ROM dele). */
export function tableRomsFor(room: GameRoom): string[] {
  const names = new Set([room.id, slug(room.id), slug(room.title)]);
  return Object.entries(ROM_GAMES).filter(([rom, games]) => names.has(rom) || games.some((g) => names.has(g))).map(([rom]) => rom);
}

/** ROMs que podem ser deste jogo: os da tabela primeiro, depois o título sem separadores. */
export function romsFor(room: GameRoom): string[] {
  const roms = tableRomsFor(room);
  for (const name of [slug(room.title), slug(room.id)]) {
    const compact = name.replace(/-/g, "");
    if (compact && compact.length <= 32 && !roms.includes(compact)) roms.push(compact);
  }
  return roms;
}
