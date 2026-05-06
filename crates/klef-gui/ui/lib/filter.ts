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
