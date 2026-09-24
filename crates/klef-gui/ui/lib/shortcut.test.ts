import { describe, expect, it } from "vitest";
import { acceleratorFromEvent, formatAccelerator } from "./shortcut";

const ev = (code: string, mods: Partial<Record<"metaKey" | "ctrlKey" | "altKey" | "shiftKey", boolean>> = {}) => ({
  code,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...mods,
});

describe("acceleratorFromEvent", () => {
  it("builds modifiers in parser order, then the physical key code", () => {
    expect(acceleratorFromEvent(ev("KeyK", { metaKey: true, shiftKey: true }))).toBe("Cmd+Shift+KeyK");
    expect(acceleratorFromEvent(ev("Digit1", { ctrlKey: true, altKey: true }))).toBe("Ctrl+Alt+Digit1");
  });

  it("ignores a lone modifier press (still recording)", () => {
    expect(acceleratorFromEvent(ev("MetaLeft", { metaKey: true }))).toBeNull();
    expect(acceleratorFromEvent(ev("ShiftRight", { shiftKey: true }))).toBeNull();
  });

  it("rejects chords that would hijack plain typing", () => {
    expect(acceleratorFromEvent(ev("KeyK"))).toBeNull();
    expect(acceleratorFromEvent(ev("KeyK", { shiftKey: true }))).toBeNull();
  });
});

describe("formatAccelerator", () => {
  it("renders macOS symbols", () => {
    expect(formatAccelerator("Cmd+Shift+KeyK")).toBe("⌘⇧K");
    expect(formatAccelerator("Ctrl+Alt+Digit1")).toBe("⌃⌥1");
    expect(formatAccelerator("Cmd+Space")).toBe("⌘Space");
  });
});
