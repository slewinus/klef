import { invoke } from "@tauri-apps/api/core";
import { loadSettings } from "./settings";
import type { KeyDto } from "./types";

// Thin wrappers over Tauri commands. Keep these typed so the Svelte
// components don't sprinkle string literals like "list_keys" everywhere.

export function listKeys(): Promise<KeyDto[]> {
  return invoke<KeyDto[]>("list_keys");
}

/**
 * Copy a key's value to the clipboard. Rust reads and writes it directly,
 * so the plaintext never enters the webview; it also schedules the
 * auto-clear and records `last_used_at`.
 */
export function copyKey(name: string): Promise<void> {
  return invoke<void>("copy_key", {
    name,
    clearAfterSecs: loadSettings().autoClearSeconds,
  });
}

export interface AddKeyInput {
  name: string;
  value: string;
  envVar?: string;
  note?: string;
  tags: string[];
}

export function addKey(input: AddKeyInput): Promise<void> {
  // Tauri serializes camelCase JS keys to snake_case Rust args
  // automatically when the Rust function uses snake_case parameter names.
  return invoke<void>("add_key", {
    name: input.name,
    value: input.value,
    envVar: input.envVar ?? null,
    note: input.note ?? null,
    tags: input.tags,
  });
}

export function deleteKey(name: string): Promise<void> {
  return invoke<void>("delete_key", { name });
}

export interface EditKeyInput {
  name: string;
  /** Rename to this first, when it differs from `name`. */
  newName?: string;
  /** undefined = preserve the existing secret value. */
  value?: string;
  envVar?: string;
  note?: string;
  tags: string[];
}

export function editKey(input: EditKeyInput): Promise<void> {
  return invoke<void>("edit_key", {
    name: input.name,
    newName: input.newName ?? null,
    value: input.value ?? null,
    envVar: input.envVar ?? null,
    note: input.note ?? null,
    tags: input.tags,
  });
}

/** Switch to an existing age vault; rejects on a wrong passphrase. */
export function openAgeVault(path: string, passphrase: string): Promise<void> {
  return invoke<void>("open_age_vault", { path, passphrase });
}

export function useKeychain(): Promise<void> {
  return invoke<void>("use_keychain");
}

/** Register the global toggle shortcut; rejects if another app owns it. */
export function setShortcut(accelerator: string): Promise<void> {
  return invoke<void>("set_shortcut", { accelerator });
}

export interface DotenvPlanItem {
  env_var: string;
  klef_name: string;
  redacted_value: string;
  /** "new" | "conflict" | "ref" | "empty" */
  status: string;
}

/** Preview plan returned by the Rust side. The plaintext values stay
 *  server-side, keyed by `session_id`; the webview never sees them. */
export interface DotenvPlan {
  session_id: string;
  suggested_project: string;
  items: DotenvPlanItem[];
  source_path: string;
}

export function previewDotenvImport(path: string): Promise<DotenvPlan> {
  return invoke<DotenvPlan>("preview_dotenv_import", { path });
}

export function applyDotenvImport(
  sessionId: string,
  project: string,
  rewriteSource: boolean,
  /** `env_var` names the user accepted. The server canonicalized the
   *  source path at preview time and ignores anything the webview tries
   *  to override. */
  accepted: string[],
): Promise<number> {
  return invoke<number>("apply_dotenv_import", {
    sessionId,
    project,
    rewriteSource,
    accepted,
  });
}

export function cancelDotenvImport(sessionId: string): Promise<void> {
  return invoke<void>("cancel_dotenv_import", { sessionId });
}
