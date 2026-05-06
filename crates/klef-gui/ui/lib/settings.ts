/**
 * App-level settings persisted in localStorage. Lightweight by design:
 * we don't want to spin up the Tauri filesystem plugin and a JSON file
 * dance for two scalar fields. localStorage is private to the webview
 * and survives across launches.
 */

const STORAGE_KEY = "klef.settings.v1";

export interface Settings {
  /** Seconds before the clipboard auto-clears after a copy. 0 disables. */
  autoClearSeconds: number;
}

export const DEFAULT_SETTINGS: Settings = {
  autoClearSeconds: 30,
};

// Autostart state lives in macOS LaunchAgents, not in localStorage. The
// plugin tracks it via ~/Library/LaunchAgents/<bundle>.plist.
export async function isAutostartEnabled(): Promise<boolean> {
  const m = await import("@tauri-apps/plugin-autostart");
  return m.isEnabled();
}

export async function setAutostart(enabled: boolean): Promise<void> {
  const m = await import("@tauri-apps/plugin-autostart");
  if (enabled) {
    await m.enable();
  } else {
    await m.disable();
  }
}

const MIN_AUTO_CLEAR = 0;
const MAX_AUTO_CLEAR = 600;

function clampAutoClear(n: number): number {
  if (!Number.isFinite(n)) return DEFAULT_SETTINGS.autoClearSeconds;
  return Math.max(MIN_AUTO_CLEAR, Math.min(MAX_AUTO_CLEAR, Math.round(n)));
}

/** Load settings from localStorage, falling back to defaults. */
export function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_SETTINGS };
    const parsed = JSON.parse(raw) as Partial<Settings>;
    return {
      autoClearSeconds: clampAutoClear(
        parsed.autoClearSeconds ?? DEFAULT_SETTINGS.autoClearSeconds,
      ),
    };
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

/** Save settings (mutates localStorage). Validates and clamps before writing. */
export function saveSettings(s: Settings): Settings {
  const sanitized: Settings = {
    autoClearSeconds: clampAutoClear(s.autoClearSeconds),
  };
  localStorage.setItem(STORAGE_KEY, JSON.stringify(sanitized));
  return sanitized;
}

/** Convert auto-clear setting to milliseconds; 0 means "no auto-clear". */
export function autoClearMs(s: Settings): number | null {
  if (s.autoClearSeconds <= 0) return null;
  return s.autoClearSeconds * 1000;
}
