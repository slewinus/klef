import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

/** Returns a getter for "is any modal open right now?". */
export type IsModalOpen = () => boolean;

/**
 * Wires the popover's lifecycle events:
 * - Calls `onShown` whenever the Rust side emits `popover-shown` (so the
 *   webview can refresh data + refocus the search bar).
 * - Auto-hides the webview window when focus leaves the app — the standard
 *   menu bar utility behavior. Suppressed while a modal is open so the
 *   modal-mount focus shift doesn't freeze the popover mid-render.
 *
 * Returns a teardown function to unsubscribe from both listeners. Intended
 * to be called from `onMount`.
 */
export async function setupPopoverLifecycle(
  onShown: () => void,
  isModalOpen: IsModalOpen,
): Promise<() => void> {
  const win = getCurrentWebviewWindow();

  const unlistenShown = await listen("popover-shown", () => onShown());

  const unlistenFocus = await win.onFocusChanged(({ payload: focused }) => {
    if (!focused && !isModalOpen()) {
      win.hide();
    }
  });

  return () => {
    unlistenShown();
    unlistenFocus();
  };
}

/** Hide the popover programmatically (used by the Escape key handler). */
export async function hideCurrentPopover(): Promise<void> {
  await getCurrentWebviewWindow().hide();
}

/**
 * Heuristic match on Keychain access denial errors. Used to decide
 * whether to show the dedicated help screen instead of a raw error.
 */
export function isKeychainDenied(err: string): boolean {
  const e = err.toLowerCase();
  return (
    e.includes("denied") ||
    e.includes("not authorized") ||
    e.includes("user did not consent")
  );
}
