import { openAgeVault, useKeychain } from "./api";
import { loadSettings, saveSettings } from "./settings";

/**
 * Which vault Rust has open vs which the settings ask for. Rust starts on
 * the Keychain every launch, so a configured age vault starts `locked`
 * until the user types its passphrase.
 */
export class Vault {
  wanted = $state(loadSettings().vaultPath);
  active = $state<string | null>(null);
  locked = $derived(this.wanted !== this.active);
  /** Basename of the open age vault, null on the Keychain. */
  name = $derived(this.active?.split("/").pop() ?? null);

  async unlock(passphrase: string): Promise<void> {
    if (this.wanted === null) return;
    await openAgeVault(this.wanted, passphrase);
    this.active = this.wanted;
  }

  /** Re-read settings after they change. Returns true when the open store
   *  switched (back to the Keychain), so the caller reloads its list. */
  async sync(): Promise<boolean> {
    this.wanted = loadSettings().vaultPath;
    if (this.wanted !== null || this.active === null) return false;
    await useKeychain();
    this.active = null;
    return true;
  }

  /** Forget the configured vault (e.g. the file moved) and go back to the
   *  Keychain. Same return contract as `sync`. */
  forget(): Promise<boolean> {
    saveSettings({ ...loadSettings(), vaultPath: null });
    return this.sync();
  }
}
