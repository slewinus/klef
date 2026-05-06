import {
  readText,
  writeText,
} from "@tauri-apps/plugin-clipboard-manager";

const DEFAULT_AUTO_CLEAR_MS = 30_000;

// Track the last value we wrote so we can later check whether the clipboard
// still holds it before clearing. This avoids wiping content the user
// manually copied from elsewhere within the timeout window.
let lastWritten: string | null = null;
let pendingClearTimer: ReturnType<typeof setTimeout> | null = null;

/**
 * Write `value` to the system clipboard and schedule an automatic clear
 * after `timeoutMs` milliseconds. The clear only fires if the clipboard
 * still contains exactly `value` at that point — otherwise we assume the
 * user copied something else and leave it alone.
 *
 * Subsequent calls cancel the pending clear from any earlier call.
 */
export async function copyWithAutoClear(
  value: string,
  timeoutMs: number = DEFAULT_AUTO_CLEAR_MS,
): Promise<void> {
  await writeText(value);
  lastWritten = value;
  if (pendingClearTimer !== null) {
    clearTimeout(pendingClearTimer);
  }
  pendingClearTimer = setTimeout(async () => {
    pendingClearTimer = null;
    try {
      const current = await readText();
      if (current === lastWritten) {
        await writeText("");
      }
    } catch {
      // Best-effort clear. If the clipboard read fails (rare; usually a
      // permission glitch), we leave the clipboard alone rather than
      // wiping it blindly.
    } finally {
      lastWritten = null;
    }
  }, timeoutMs);
}
