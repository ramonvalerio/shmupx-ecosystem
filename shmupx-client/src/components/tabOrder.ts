/** Move `id` para a posição `to` (índice final). */
export function moveTab(ids: string[], id: string, to: number): string[] {
  const rest = ids.filter((other) => other !== id);
  const index = Math.max(0, Math.min(to, rest.length));
  return [...rest.slice(0, index), id, ...rest.slice(index)];
}
