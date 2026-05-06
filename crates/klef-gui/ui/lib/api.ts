import { invoke } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import type { KeyDto } from "./types";

// Thin wrappers over Tauri commands. Keep these typed so the Svelte
// components don't sprinkle string literals like "list_keys" everywhere.

export function listKeys(): Promise<KeyDto[]> {
  return invoke<KeyDto[]>("list_keys");
}

export function getKeyValue(name: string): Promise<string> {
  return invoke<string>("get_key_value", { name });
}

export function copyToClipboard(value: string): Promise<void> {
  return writeText(value);
}
