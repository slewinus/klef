import { open as openDialog } from "@tauri-apps/plugin-dialog";

/**
 * Open a native file picker for `.env` import. Resolves to the chosen
 * absolute path, or `null` if the user cancelled.
 *
 * Used as a fallback to drag-drop for editors (VS Code, Antigravity, ...)
 * whose drag pasteboard format Tauri can't decode into a file path.
 */
export async function pickDotenvFile(): Promise<string | null> {
  const path = await openDialog({
    multiple: false,
    filters: [{ name: "dotenv", extensions: ["env", "*"] }],
    title: "Import a .env file",
  });
  return typeof path === "string" ? path : null;
}
