import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// Provide a minimal localStorage shim — vitest's default environment is
// `node` and doesn't ship one. We swap it per-test.
const memoryStorage = (() => {
  let store: Record<string, string> = {};
  return {
    getItem: (k: string) => (k in store ? store[k] : null),
    setItem: (k: string, v: string) => {
      store[k] = v;
    },
    removeItem: (k: string) => {
      delete store[k];
    },
    clear: () => {
      store = {};
    },
  };
})();
vi.stubGlobal("localStorage", memoryStorage);

const { DEFAULT_SETTINGS, loadSettings, saveSettings } = await import("./settings");

beforeEach(() => {
  memoryStorage.clear();
});

afterEach(() => {
  memoryStorage.clear();
});

describe("loadSettings", () => {
  it("returns defaults when no entry exists", () => {
    expect(loadSettings()).toEqual(DEFAULT_SETTINGS);
  });

  it("returns defaults when JSON is malformed", () => {
    memoryStorage.setItem("klef.settings.v1", "{not json");
    expect(loadSettings()).toEqual(DEFAULT_SETTINGS);
  });

  it("clamps overly-large values to the max (600s)", () => {
    memoryStorage.setItem(
      "klef.settings.v1",
      JSON.stringify({ autoClearSeconds: 9999 }),
    );
    expect(loadSettings().autoClearSeconds).toBe(600);
  });

  it("clamps negative values to 0 (disabled)", () => {
    memoryStorage.setItem(
      "klef.settings.v1",
      JSON.stringify({ autoClearSeconds: -10 }),
    );
    expect(loadSettings().autoClearSeconds).toBe(0);
  });

  it("falls back to default when field is missing", () => {
    memoryStorage.setItem("klef.settings.v1", JSON.stringify({}));
    expect(loadSettings()).toEqual(DEFAULT_SETTINGS);
  });
});

describe("saveSettings", () => {
  it("persists and round-trips", () => {
    saveSettings({ ...DEFAULT_SETTINGS, autoClearSeconds: 60 });
    expect(loadSettings().autoClearSeconds).toBe(60);
  });

  it("clamps + rounds before storing", () => {
    const result = saveSettings({ ...DEFAULT_SETTINGS, autoClearSeconds: 12.7 });
    expect(result.autoClearSeconds).toBe(13);
  });

  it("stores 0 (disabled) without coercing to default", () => {
    saveSettings({ ...DEFAULT_SETTINGS, autoClearSeconds: 0 });
    expect(loadSettings().autoClearSeconds).toBe(0);
  });
});

describe("shortcut and vault", () => {
  it("default to ⌘⇧K and the keychain", () => {
    expect(loadSettings().shortcut).toBe("Cmd+Shift+KeyK");
    expect(loadSettings().vaultPath).toBeNull();
  });

  it("round-trip", () => {
    saveSettings({ ...DEFAULT_SETTINGS, shortcut: "Ctrl+Alt+KeyP", vaultPath: "/v/klef.age" });
    expect(loadSettings().shortcut).toBe("Ctrl+Alt+KeyP");
    expect(loadSettings().vaultPath).toBe("/v/klef.age");
  });

  it("drop wrong-typed stored values back to defaults", () => {
    memoryStorage.setItem(
      "klef.settings.v1",
      JSON.stringify({ shortcut: 42, vaultPath: "" }),
    );
    expect(loadSettings().shortcut).toBe("Cmd+Shift+KeyK");
    expect(loadSettings().vaultPath).toBeNull();
  });
});
