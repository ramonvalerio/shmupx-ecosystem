/**
 * Links nas mensagens do chat: `https://…`, `http://…` e `www.…` viram partes clicáveis (abrem no
 * navegador do sistema). Pontuação no fim (".", ",", ")"…) fica fora do link. Sem dependências:
 * os testes (tests/ui-state.test.mjs) carregam este arquivo direto.
 */
export type ChatPart = { text: string } | { text: string; url: string };

const LINK = /\b(?:https?:\/\/|www\.)[^\s<>"]+/gi;

export function linkParts(text: string): ChatPart[] {
  const parts: ChatPart[] = [];
  let last = 0;
  for (const match of text.matchAll(LINK)) {
    let link = match[0];
    // Pontuação colada no fim e parêntese que não abriu dentro do link ficam de fora.
    while (/[.,;:!?'\]]$/.test(link) || (link.endsWith(")") && (link.match(/\(/g)?.length ?? 0) < (link.match(/\)/g)?.length ?? 0))) {
      link = link.slice(0, -1);
    }
    const start = match.index ?? 0;
    if (start > last) parts.push({ text: text.slice(last, start) });
    parts.push({ text: link, url: link.toLowerCase().startsWith("www.") ? `https://${link}` : link });
    last = start + link.length;
  }
  if (last < text.length) parts.push({ text: text.slice(last) });
  return parts;
}
