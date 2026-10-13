/** Wrap matched characters so the launcher can show the fuzzy hit without HTML injection. */
export function highlightParts(title: string, indices: number[]): { text: string; hit: boolean }[] {
  const chars = Array.from(title);
  const hits = new Set(indices.filter((i) => i >= 0 && i < chars.length));
  const parts: { text: string; hit: boolean }[] = [];
  chars.forEach((ch, i) => {
    const hit = hits.has(i);
    const last = parts[parts.length - 1];
    if (last && last.hit === hit) last.text += ch;
    else parts.push({ text: ch, hit });
  });
  return parts;
}
