/**
 * Filtro de palavrões do chat da sala (português, inglês e japonês). Troca a palavra por `*`, do
 * mesmo tamanho. Roda ao enviar (a mensagem já é gravada limpa) e ao exibir (vale também para
 * mensagens antigas ou de quem burlou o envio).
 *
 * Português e inglês: palavra inteira, sem acento, em minúsculas, com números no lugar de letras
 * (`f0da`, `sh1t`) e letras repetidas (`porraaa`). Palavras comuns que contêm um termo não são
 * afetadas ("assistir", "cockpit", "curso"). Algumas raízes valem com sufixo (`fuck` pega
 * `fucking`). Japonês não separa palavras: procura o termo dentro do texto, só com termos que
 * não aparecem por acaso em palavras comuns (ex.: `くそ` fica de fora por causa de `やくそく`).
 */

/** Palavras inteiras (já sem acento). */
const WORDS = [
  // Português
  "porra", "caralho", "caraio", "krl", "crl", "buceta", "boceta", "xoxota", "xereca", "puta", "putas", "puto", "putos",
  "putaria", "foder", "fode", "fodeu", "fodase", "foda", "fodas", "fodido", "fodida", "fudido", "fudida", "fuder", "fudeu",
  "merda", "merdas", "bosta", "bostas", "cacete", "kct", "cu", "cus", "cuzao", "cuzinho", "arrombado", "arrombada",
  "viado", "viados", "viadinho", "corno", "cornos", "vadia", "vadias", "vagabunda", "vagabundo", "otario", "otaria",
  "babaca", "punheta", "punheteiro", "siririca", "desgracado", "desgracada", "retardado", "retardada", "fdp", "pqp",
  "vsf", "vtnc", "tnc", "prr", "poha", "cusao", "pau no cu",
  // Inglês
  "ass", "asses", "dick", "dicks", "cock", "cocks", "pussy", "pussies", "fag", "fags", "twat", "cum", "nigga", "niggas",
  "jackass", "douche", "douchebag", "bollocks", "wtf", "stfu", "fck", "fk", "fuk", "retard", "retards", "retarded",
  "arrombados", "arrombadas", "desgracados", "desgracadas",
];

/** Raízes que valem com qualquer final (ex.: `fucking`, `bitches`, `merdinha`). */
const STEMS = [
  "fuck", "motherfuck", "shit", "bitch", "asshole", "bastard", "cunt", "dickhead", "faggot", "nigger", "whore",
  "slut", "wank", "caralh", "putari", "merdinh", "punhet", "vagabund", "otari",
];

/** Japonês: procurados dentro do texto. */
const JAPANESE = [
  "死ね", "氏ね", "ぶっ殺す", "殺すぞ", "クソ", "糞", "ちんこ", "チンコ", "ちんぽ", "チンポ", "まんこ", "マンコ",
  "きちがい", "キチガイ", "気違い", "ガイジ", "ファック", "ビッチ", "くたばれ", "レイプ", "強姦", "売女", "淫売",
];

const LEET: Record<string, string> = { "0": "o", "1": "i", "3": "e", "4": "a", "5": "s", "7": "t", "@": "a", "$": "s" };

/** Minúsculas, sem acento, com números/símbolos trocados pelas letras. */
function normalize(word: string): string {
  return word
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[013457@$]/g, (c) => LEET[c] ?? c);
}

/** Letras repetidas viram uma só (`porraaa` -> `pora`); compara com a lista também colapsada. */
const collapse = (word: string) => word.replace(/(.)\1+/g, "$1");

const WORD_SET = new Set(WORDS.filter((w) => !w.includes(" ")).map(normalize));
const WORD_SET_COLLAPSED = new Set([...WORD_SET].map(collapse));
const STEMS_N = STEMS.map(normalize);
const STEMS_COLLAPSED = STEMS_N.map(collapse);

function isBadWord(token: string): boolean {
  const word = normalize(token);
  if (WORD_SET.has(word) || STEMS_N.some((stem) => word.startsWith(stem))) return true;
  // Só quem tinha letras repetidas compara pela forma colapsada: "as" não vira "ass".
  const collapsed = collapse(word);
  if (collapsed === word) return false;
  return WORD_SET_COLLAPSED.has(collapsed) || STEMS_COLLAPSED.some((stem) => collapsed.startsWith(stem));
}

const mask = (text: string) => "*".repeat([...text].length);

/** Texto com os palavrões trocados por `*`; `changed` diz se algo foi ocultado. */
export function censor(text: string): { text: string; changed: boolean } {
  let changed = false;
  // Palavras em alfabeto latino (com números e @/$, para pegar f0da, @ss, $hit).
  let result = text.replace(/[\p{L}\p{N}@$]+/gu, (token) => {
    if (!/[A-Za-zÀ-ÿ]/.test(token) && !/[@$]/.test(token)) return token;
    if (!isBadWord(token)) return token;
    changed = true;
    return mask(token);
  });
  for (const term of JAPANESE) {
    if (result.includes(term)) {
      changed = true;
      result = result.split(term).join("＊".repeat([...term].length));
    }
  }
  return { text: result, changed };
}
