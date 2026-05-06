import { invoke } from "@tauri-apps/api/core";
import { copyWithAutoClear } from "./clipboard";
import type { KeyDto } from "./types";

// Thin wrappers over Tauri commands. Keep these typed so the Svelte
// components don't sprinkle string literals like "list_keys" everywhere.

export function listKeys(): Promise<KeyDto[]> {
  return invoke<KeyDto[]>("list_keys");
}

export function getKeyValue(name: string): Promise<string> {
  return invoke<string>("get_key_value", { name });
}

// Copies the value AND schedules an auto-clear after 30 s. See
// `./clipboard.ts` for the semantics around concurrent copies and
// best-effort clearing.
export function copyToClipboard(value: string): Promise<void> {
  return copyWithAutoClear(value);
}
