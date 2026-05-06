import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// Mock the Tauri clipboard plugin before importing the module under test.
// `vi.hoisted` makes the mock state available to both the factory and the
// test bodies.
const mocks = vi.hoisted(() => {
  let clipboard = "";
  return {
    writeText: vi.fn(async (value: string) => {
      clipboard = value;
    }),
    readText: vi.fn(async () => clipboard),
    setClipboard: (v: string) => {
      clipboard = v;
    },
    getClipboard: () => clipboard,
    reset: () => {
      clipboard = "";
    },
  };
});

vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({
  writeText: mocks.writeText,
  readText: mocks.readText,
}));

const { copyWithAutoClear } = await import("./clipboard");

beforeEach(() => {
  vi.useFakeTimers();
  mocks.reset();
  mocks.writeText.mockClear();
  mocks.readText.mockClear();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("copyWithAutoClear", () => {
  it("writes the value to the clipboard", async () => {
    await copyWithAutoClear("secret-1", 1000);
    expect(mocks.writeText).toHaveBeenCalledWith("secret-1");
    expect(mocks.getClipboard()).toBe("secret-1");
  });

  it("clears the clipboard after the timeout if value is unchanged", async () => {
    await copyWithAutoClear("secret-1", 1000);
    expect(mocks.getClipboard()).toBe("secret-1");

    await vi.advanceTimersByTimeAsync(1000);

    expect(mocks.getClipboard()).toBe("");
    // Two writes total: the initial copy + the clear.
    expect(mocks.writeText).toHaveBeenCalledTimes(2);
    expect(mocks.writeText).toHaveBeenLastCalledWith("");
  });

  it("does not clear if the user copied something else within the window", async () => {
    await copyWithAutoClear("secret-1", 1000);

    // Simulate the user manually copying something else (e.g. ⌘C in Notes).
    mocks.setClipboard("user manual copy");

    await vi.advanceTimersByTimeAsync(1000);

    // Clipboard should still hold the user's manual copy, untouched.
    expect(mocks.getClipboard()).toBe("user manual copy");
  });

  it("cancels the previous timer when a new copy happens (latest-wins)", async () => {
    await copyWithAutoClear("secret-1", 1000);
    await vi.advanceTimersByTimeAsync(500); // half-way through the first timer
    await copyWithAutoClear("secret-2", 1000);

    // 600ms after the second copy — the first timer would have fired at
    // t=1000ms (relative to first copy = 500ms after second copy here)
    // and would have wiped secret-2 if it weren't cancelled.
    await vi.advanceTimersByTimeAsync(600);
    expect(mocks.getClipboard()).toBe("secret-2");

    // 400ms more = 1000ms after second copy — its timer fires.
    await vi.advanceTimersByTimeAsync(400);
    expect(mocks.getClipboard()).toBe("");
  });

  it("uses 30s by default", async () => {
    await copyWithAutoClear("secret-1");

    await vi.advanceTimersByTimeAsync(29_000);
    expect(mocks.getClipboard()).toBe("secret-1");

    await vi.advanceTimersByTimeAsync(1_000);
    expect(mocks.getClipboard()).toBe("");
  });

  it("survives readText errors without wiping", async () => {
    mocks.readText.mockRejectedValueOnce(new Error("clipboard unavailable"));
    await copyWithAutoClear("secret-1", 1000);

    await vi.advanceTimersByTimeAsync(1000);

    // readText threw, so we don't know the state and leave it alone.
    // The clipboard value here is whatever writeText set it to ("secret-1")
    // since we never called writeText("") on the failure path.
    expect(mocks.getClipboard()).toBe("secret-1");
  });
});
