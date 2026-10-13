/** Place `movingId` among `visibleIds`, then write that order back into `allIds`.
 * Apps that are not in the visible list stay in their slots.
 * Dragging downward lands after the target. Dragging upward lands before it.
 */
export function reorderVisible(
  allIds: readonly string[],
  visibleIds: readonly string[],
  movingId: string,
  targetId: string,
): string[] {
  const from = visibleIds.indexOf(movingId);
  const to = visibleIds.indexOf(targetId);
  if (from < 0 || to < 0 || from === to) return [...allIds];
  const nextVisible = visibleIds.filter((id) => id !== movingId);
  const targetIndex = nextVisible.indexOf(targetId);
  if (targetIndex < 0) return [...allIds];
  const insertAt = from < to ? targetIndex + 1 : targetIndex;
  nextVisible.splice(insertAt, 0, movingId);
  const visible = new Set(visibleIds);
  let cursor = 0;
  return allIds.map((id) => {
    if (!visible.has(id)) return id;
    const replacement = nextVisible[cursor];
    cursor += 1;
    return replacement ?? id;
  });
}
