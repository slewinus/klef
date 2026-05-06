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
  /** undefined = preserve the existing secret value. */
  value?: string;
  envVar?: string;
  note?: string;
  tags: string[];
}

export function editKey(input: EditKeyInput): Promise<void> {
  return invoke<void>("edit_key", {
    name: input.name,
    value: input.value ?? null,
    envVar: input.envVar ?? null,
    note: input.note ?? null,
    tags: input.tags,
  });
}

// Copies the value AND schedules an auto-clear after 30 s. See
// `./clipboard.ts` for the semantics around concurrent copies and
// best-effort clearing.
export function copyToClipboard(value: string): Promise<void> {
  return copyWithAutoClear(value);
}
