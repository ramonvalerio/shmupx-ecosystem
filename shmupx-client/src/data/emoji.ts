/**
 * Emojis do chat e os atalhos deles: `:nome:` (com sugestões ao digitar `:` e duas letras) e os
 * emoticons clássicos (`:)`, `<3`, `xD`…), que viram emoji ao digitar espaço depois ou ao enviar.
 * Sem dependências: os testes (tests/ui-state.test.mjs) carregam este arquivo direto.
 */
export interface ChatEmoji {
  emoji: string;
  /** Nomes do atalho `:nome:`; o primeiro aparece na dica do botão. */
  names: string[];
}

export const CHAT_EMOJIS: ChatEmoji[] = [
  { emoji: "😀", names: ["sorriso", "smile"] },
  { emoji: "😂", names: ["rindo", "joy", "kkk"] },
  { emoji: "🤣", names: ["rolando", "rofl"] },
  { emoji: "😅", names: ["ufa", "sweat_smile"] },
  { emoji: "😉", names: ["piscada", "wink"] },
  { emoji: "😊", names: ["feliz", "blush"] },
  { emoji: "😎", names: ["oculos", "cool"] },
  { emoji: "😍", names: ["apaixonado", "love"] },
  { emoji: "🤩", names: ["estrelas", "starstruck"] },
  { emoji: "🥳", names: ["festa", "party"] },
  { emoji: "😮", names: ["uau", "wow"] },
  { emoji: "😱", names: ["susto", "scream"] },
  { emoji: "🤯", names: ["explodiu", "mindblown"] },
  { emoji: "🤔", names: ["pensando", "thinking"] },
  { emoji: "😴", names: ["sono", "sleep"] },
  { emoji: "😢", names: ["choro", "cry"] },
  { emoji: "😭", names: ["chorando", "sob"] },
  { emoji: "😡", names: ["bravo", "rage"] },
  { emoji: "😤", names: ["bufando", "triumph"] },
  { emoji: "💀", names: ["caveira", "skull", "morri"] },
  { emoji: "👀", names: ["olhos", "eyes"] },
  { emoji: "👍", names: ["joinha", "ok", "thumbsup"] },
  { emoji: "👎", names: ["deslike", "thumbsdown"] },
  { emoji: "👏", names: ["palmas", "clap"] },
  { emoji: "🙌", names: ["aleluia", "raised_hands"] },
  { emoji: "🙏", names: ["valeu", "pray"] },
  { emoji: "💪", names: ["forca", "muscle"] },
  { emoji: "👋", names: ["oi", "wave"] },
  { emoji: "❤️", names: ["coracao", "heart"] },
  { emoji: "💔", names: ["coracao_partido", "broken_heart"] },
  { emoji: "🔥", names: ["fogo", "fire"] },
  { emoji: "💯", names: ["cem", "100"] },
  { emoji: "✨", names: ["brilho", "sparkles"] },
  { emoji: "⭐", names: ["estrela", "star"] },
  { emoji: "💥", names: ["boom", "explosao"] },
  { emoji: "🎉", names: ["parabens", "tada"] },
  { emoji: "🏆", names: ["trofeu", "trophy", "gg"] },
  { emoji: "🥇", names: ["ouro", "first"] },
  { emoji: "🎮", names: ["controle", "game"] },
  { emoji: "🕹️", names: ["arcade", "joystick"] },
  { emoji: "🚀", names: ["foguete", "rocket"] },
  { emoji: "👾", names: ["invader", "alien"] },
  { emoji: "✅", names: ["certo", "check"] },
  { emoji: "❌", names: ["errado", "x"] },
];

/** Emoticons clássicos (palavra inteira, separada por espaço). */
export const EMOTICONS: Record<string, string> = {
  ":)": "🙂", ":-)": "🙂", ":D": "😃", ":-D": "😃", "xD": "😆", "XD": "😆", ";)": "😉", ";-)": "😉",
  ":(": "🙁", ":-(": "🙁", ":'(": "😢", ":P": "😛", ":p": "😛", ":-P": "😛", ":O": "😮", ":o": "😮",
  ":*": "😘", ":|": "😐", "B)": "😎", "<3": "❤️", "</3": "💔",
};

const BY_NAME = new Map(CHAT_EMOJIS.flatMap((e) => e.names.map((name) => [name, e.emoji] as const)));

/** Troca `:nome:` conhecido pelo emoji (os desconhecidos ficam como estão). */
export function replaceNamed(text: string): string {
  return text.replace(/:([a-z0-9_]+):/gi, (whole, name: string) => BY_NAME.get(name.toLowerCase()) ?? whole);
}

/** Troca os emoticons que são palavras inteiras (não mexe em links nem em horários). */
export function replaceEmoticons(text: string): string {
  return text.replace(/(^|\s)(\S+)(?=\s|$)/g, (whole, space: string, word: string) => (EMOTICONS[word] ? space + EMOTICONS[word] : whole));
}

/** Converte tudo na hora de enviar. */
export function emojify(text: string): string {
  return replaceEmoticons(replaceNamed(text));
}

/**
 * Enquanto digita: troca um `:nome:` recém-fechado e o emoticon que acabou de ganhar um espaço
 * depois. Devolve o texto novo (o mesmo, se nada mudou).
 */
export function emojifyTyping(text: string): string {
  let out = replaceNamed(text);
  const match = out.match(/(^|\s)(\S+)(\s)$/);
  if (match && EMOTICONS[match[2]]) out = out.slice(0, out.length - match[2].length - 1) + EMOTICONS[match[2]] + match[3];
  return out;
}

/** Sugestões para o `:par` que está sendo digitado no fim do texto (até `limit`). */
export function emojiSuggestions(text: string, limit = 6): { query: string; items: { emoji: string; name: string }[] } | null {
  const match = text.match(/(?:^|\s):([a-z0-9_]{2,})$/i);
  if (!match) return null;
  const query = match[1].toLowerCase();
  const items: { emoji: string; name: string }[] = [];
  for (const e of CHAT_EMOJIS) {
    const name = e.names.find((n) => n.startsWith(query)) ?? e.names.find((n) => n.includes(query));
    if (name) items.push({ emoji: e.emoji, name });
  }
  // Primeiro os que começam com o digitado, e entre eles o nome mais curto (:fo → :fogo:).
  items.sort((a, b) => Number(!a.name.startsWith(query)) - Number(!b.name.startsWith(query)) || a.name.length - b.name.length);
  return items.length ? { query, items: items.slice(0, limit) } : null;
}

/** Aplica a sugestão escolhida: troca o `:par` do fim pelo emoji e um espaço. */
export function applySuggestion(text: string, query: string, emoji: string): string {
  return text.slice(0, text.length - query.length - 1) + emoji + " ";
}
