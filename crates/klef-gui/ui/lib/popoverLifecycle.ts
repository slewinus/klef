import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

/** Returns a getter for "is any modal open right now?". */
export type IsModalOpen = () => boolean;

/** Returns a getter for "is the popover pinned (auto-hide disabled)?". */
export type IsPinned = () => boolean;

/**
 * Module-level flag set while a drag operation is in progress over the
 * popover window. Auto-hide-on-blur consults this so the focus shift
 * triggered by macOS during drag-drop doesn't hide the popover before
 * the drop event fires.
 */
let dragInProgress = false;

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
  isPinned: IsPinned,
): Promise<() => void> {
  const win = getCurrentWebviewWindow();

  const unlistenShown = await listen("popover-shown", () => onShown());

  const unlistenFocus = await win.onFocusChanged(({ payload: focused }) => {
    if (!focused && !isModalOpen() && !isPinned() && !dragInProgress) {
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

/**
 * Subscribe to file-drop events on the current webview window. Returns a
 * teardown function. The callback receives only `.env` paths — other files
 * are silently ignored.
 */
export async function onDotenvDropped(
  cb: (path: string) => void,
): Promise<() => void> {
  const win = getCurrentWebviewWindow();
  const unlisten = await win.onDragDropEvent((evt) => {
    // Track drag state so auto-hide-on-blur doesn't fire while the user
    // is dragging from another app (focus shifts during drag/drop).
    if (evt.payload.type === "enter" || evt.payload.type === "over") {
      dragInProgress = true;
      return;
    }
    if (evt.payload.type === "leave") {
      dragInProgress = false;
      return;
    }
    if (evt.payload.type !== "drop") return;
    dragInProgress = false;
    for (const path of evt.payload.paths) {
      const lower = path.toLowerCase();
      if (lower.endsWith(".env") || lower.includes("/.env")) {
        cb(path);
        return;
      }
    }
  });
  return unlisten;
}
