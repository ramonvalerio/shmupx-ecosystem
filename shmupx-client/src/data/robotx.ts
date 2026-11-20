/**
 * RobotX por perguntas e respostas curadas (ADR 006): o admin cadastra no Lab, o catálogo traz
 * junto com o jogo e a resposta sai aqui, no PC do jogador, sem IA e sem custo. A busca compara
 * as palavras da pergunta do jogador com as de cada pergunta cadastrada.
 */

export interface RobotxItem {
  question: string;
  answer: string;
}

export interface RobotxReply {
  text: string;
  /** Pergunta cadastrada que respondeu; ausente quando o RobotX não encontrou nada. */
  matched?: string;
}

/** Palavras que não ajudam a distinguir perguntas (pt e en). */
const STOPWORDS = new Set(
  ("a o os as um uma uns umas de do da dos das no na nos nas em por para pra pro com sem que qual quais " +
    "quem como onde quando e ou se eu tu ele ela voce vc meu minha seu sua isso esse essa este esta aqui " +
    "ai la tem ter ha sao e ser foi vai mais muito oi ola robotx me te nos jogo jogar " +
    "the a an of to in on for with what which who how where when is are do does i you my your it this that hi hello")
    .split(" "),
);

function normalize(text: string): string {
  return text.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase().replace(/@robotx\b/g, " ");
}

/** Palavras relevantes, com plural simples reduzido ("naves" → "nave"). */
export function tokens(text: string): string[] {
  return normalize(text)
    .split(/[^a-z0-9]+/)
    .filter((w) => w.length > 1 && !STOPWORDS.has(w))
    .map((w) => (w.length > 4 && w.endsWith("s") ? w.slice(0, -1) : w));
}

/** Semelhança entre dois conjuntos de palavras: acertos sobre as palavras da pergunta cadastrada. */
function similarity(asked: Set<string>, candidate: string[]): number {
  if (candidate.length === 0 || asked.size === 0) return 0;
  const unique = new Set(candidate);
  let hits = 0;
  for (const word of unique) {
    if (asked.has(word)) hits += 1;
    // Palavra parecida (prefixo comum longo): "controle" x "controles", "chefao" x "chefoes".
    else if ([...asked].some((a) => a.length >= 5 && word.length >= 5 && a.slice(0, 5) === word.slice(0, 5))) hits += 0.7;
  }
  // Média entre cobrir a pergunta cadastrada e cobrir o que o jogador perguntou.
  return (hits / unique.size + hits / asked.size) / 2;
}

const THRESHOLD = 0.34;

/** Melhor resposta para a pergunta; sem uma boa, sugere perguntas que o RobotX conhece. */
export function answer(question: string, items: RobotxItem[]): RobotxReply {
  const asked = new Set(tokens(question));
  let best: { item: RobotxItem; score: number } | null = null;
  for (const item of items) {
    const score = similarity(asked, tokens(item.question)) + 0.15 * similarity(asked, tokens(item.answer));
    if (!best || score > best.score) best = { item, score };
  }
  if (best && best.score >= THRESHOLD) return { text: best.item.answer, matched: best.item.question };
  const examples = items.slice(0, 3).map((i) => `“${i.question}”`).join(", ");
  return { text: examples ? `Ainda não sei responder isso. Tente perguntar, por exemplo: ${examples}` : "Ainda não sei responder isso." };
}

/** Lê o campo `robotxQa` do documento do jogo (Firestore), ignorando itens malformados. */
export function robotxFromGame(value: unknown): RobotxItem[] {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => (item && typeof item === "object" ? (item as Record<string, unknown>) : {}))
    .filter((item) => typeof item.q === "string" && typeof item.a === "string" && item.q && item.a)
    .map((item) => ({ question: item.q as string, answer: item.a as string }));
}
