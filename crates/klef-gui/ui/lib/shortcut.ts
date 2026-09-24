// Global-shortcut recording. The accelerator string is parsed on the Rust
// side by `global-hotkey`, which accepts `KeyboardEvent.code` values
// (KeyK, Digit1, ArrowUp, Space…) as the main key.

export const DEFAULT_SHORTCUT = "Cmd+Shift+KeyK";

type ChordEvent = Pick<KeyboardEvent, "code" | "metaKey" | "ctrlKey" | "altKey" | "shiftKey">;

/** Accelerator for a keydown, or null while only modifiers are held or when
 *  the chord has no Cmd/Ctrl/Alt (a bare or Shift-only key would fire on
 *  ordinary typing in every app). */
export function acceleratorFromEvent(e: ChordEvent): string | null {
  if (/^(Meta|Control|Alt|Shift)(Left|Right)?$/.test(e.code)) return null;
  if (!e.metaKey && !e.ctrlKey && !e.altKey) return null;
  const mods = [
    e.metaKey && "Cmd",
    e.ctrlKey && "Ctrl",
    e.altKey && "Alt",
    e.shiftKey && "Shift",
  ].filter(Boolean);
  return [...mods, e.code].join("+");
}

const SYMBOLS: Record<string, string> = { Cmd: "⌘", Ctrl: "⌃", Alt: "⌥", Shift: "⇧" };

/** "Cmd+Shift+KeyK" → "⌘⇧K" for display. */
export function formatAccelerator(accel: string): string {
  return accel
    .split("+")
    .map((t) => SYMBOLS[t] ?? t.replace(/^(Key|Digit)/, ""))
    .join("");
}
