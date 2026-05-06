import type { KeyDto } from "./types";

/**
 * Case-insensitive substring filter on name, env_var, note, and tags.
 *
 * Substring (not fuzzy) is the right call for a vault: `klef list --filter`
 * uses substring too, and users typing `stripe` expect every key whose name
 * literally contains "stripe", not a fuzzy similarity to "Cloudstrike". If
 * we ever add real fuzzy ranking it should be opt-in (`?` prefix?) so the
 * exact-match flow stays predictable.
 */
export function filterKeys(keys: KeyDto[], query: string): KeyDto[] {
  const q = query.trim().toLowerCase();
  if (q === "") return keys;
  return keys.filter((k) => {
    if (k.name.toLowerCase().includes(q)) return true;
    if (k.env_var.toLowerCase().includes(q)) return true;
    if (k.note && k.note.toLowerCase().includes(q)) return true;
    if (k.tags && k.tags.some((t) => t.toLowerCase().includes(q))) return true;
    return false;
  });
}

/**
 * Keep only keys tagged `project:<name>` for the given project. Returns the
 * input unchanged when no project is selected.
 */
export function filterByProject(
  keys: KeyDto[],
  project: string | null,
): KeyDto[] {
  if (project === null) return keys;
  const tag = `project:${project}`;
  return keys.filter((k) => k.tags?.includes(tag));
}

/**
 * Sort keys by `last_used_at` descending (most recently used first).
 * Keys without a timestamp fall to the bottom in alphabetical order.
 * Returns a new array; does not mutate the input.
 */
export function sortByLastUsed(keys: KeyDto[]): KeyDto[] {
  return [...keys].sort((a, b) => {
    const ta = a.last_used_at;
    const tb = b.last_used_at;
    if (ta && tb) return tb.localeCompare(ta);
    if (ta) return -1;
    if (tb) return 1;
    return a.name.localeCompare(b.name);
  });
}
