/**
 * Handle the arrow-key navigation for a list of items.
 * Returns the new index after applying the keypress, or the same index if
 * the key wasn't an arrow.
 *
 * Wraps around at the boundaries so Down at the last item lands on the
 * first, and Up at the first lands on the last — matches Spotlight,
 * Raycast, and similar launchers.
 */
export function nextIndexFor(
  key: string,
  current: number,
  length: number,
): number {
  if (length === 0) return 0;
  if (key === "ArrowDown") return (current + 1) % length;
  if (key === "ArrowUp") return (current - 1 + length) % length;
  return current;
}

/** Returns true if the key should consume the event (preventDefault). */
export function isNavKey(key: string): boolean {
  return key === "ArrowDown" || key === "ArrowUp";
}
