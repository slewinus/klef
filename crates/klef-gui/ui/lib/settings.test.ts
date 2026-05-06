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

const {
  DEFAULT_SETTINGS,
  autoClearMs,
  loadSettings,
  saveSettings,
} = await import("./settings");

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
    saveSettings({ autoClearSeconds: 60 });
    expect(loadSettings().autoClearSeconds).toBe(60);
  });

  it("clamps + rounds before storing", () => {
    const result = saveSettings({ autoClearSeconds: 12.7 });
    expect(result.autoClearSeconds).toBe(13);
  });

  it("stores 0 (disabled) without coercing to default", () => {
    saveSettings({ autoClearSeconds: 0 });
    expect(loadSettings().autoClearSeconds).toBe(0);
  });
});

describe("autoClearMs", () => {
  it("returns null when disabled (0 seconds)", () => {
    expect(autoClearMs({ autoClearSeconds: 0 })).toBeNull();
  });

  it("converts seconds to milliseconds", () => {
    expect(autoClearMs({ autoClearSeconds: 30 })).toBe(30_000);
    expect(autoClearMs({ autoClearSeconds: 5 })).toBe(5_000);
  });
});
