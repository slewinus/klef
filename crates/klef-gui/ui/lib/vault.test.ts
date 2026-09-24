import { beforeEach, describe, expect, it, vi } from "vitest";

const store: Record<string, string> = {};
vi.stubGlobal("localStorage", {
  getItem: (k: string) => store[k] ?? null,
  setItem: (k: string, v: string) => void (store[k] = v),
});
const api = vi.hoisted(() => ({
  openAgeVault: vi.fn(async (_p: string, pass: string) => {
    if (pass !== "right") throw new Error("wrong passphrase, or not an age vault");
  }),
  useKeychain: vi.fn(async () => {}),
}));
vi.mock("./api", () => api);

const { Vault } = await import("./vault.svelte");
const { DEFAULT_SETTINGS, loadSettings, saveSettings } = await import("./settings");

const configure = (vaultPath: string | null) => saveSettings({ ...DEFAULT_SETTINGS, vaultPath });

beforeEach(() => {
  configure(null);
  vi.clearAllMocks();
});

describe("Vault", () => {
  it("is unlocked on the Keychain by default", () => {
    const v = new Vault();
    expect(v.locked).toBe(false);
    expect(v.name).toBeNull();
  });

  it("starts locked when an age vault is configured, unlocks with the right passphrase", async () => {
    configure("/v/team.age");
    const v = new Vault();
    expect(v.locked).toBe(true);
    await expect(v.unlock("nope")).rejects.toThrow("wrong passphrase");
    expect(v.locked).toBe(true);
    await v.unlock("right");
    expect(v.locked).toBe(false);
    expect(v.name).toBe("team.age");
  });

  it("switches Rust back to the Keychain when the vault setting is cleared", async () => {
    configure("/v/team.age");
    const v = new Vault();
    await v.unlock("right");
    configure(null);
    expect(await v.sync()).toBe(true);
    expect(api.useKeychain).toHaveBeenCalledOnce();
    expect(v.locked).toBe(false);
    expect(v.name).toBeNull();
  });

  it("forget() drops the setting and leaves an open vault for the Keychain", async () => {
    configure("/v/team.age");
    const v = new Vault();
    await v.unlock("right");
    configure("/v/other.age");
    await v.sync();
    expect(v.locked).toBe(true); // wants other.age, still has team.age open
    expect(await v.forget()).toBe(true);
    expect(loadSettings().vaultPath).toBeNull();
    expect(api.useKeychain).toHaveBeenCalledOnce();
    expect(v.locked).toBe(false);
  });
});
