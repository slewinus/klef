import { describe, expect, it } from "vitest";
import { isNavKey, nextIndexFor } from "./keyboardNav";

describe("isNavKey", () => {
  it("recognizes ArrowDown and ArrowUp", () => {
    expect(isNavKey("ArrowDown")).toBe(true);
    expect(isNavKey("ArrowUp")).toBe(true);
  });

  it("rejects other keys", () => {
    expect(isNavKey("Enter")).toBe(false);
    expect(isNavKey("Escape")).toBe(false);
    expect(isNavKey("a")).toBe(false);
    expect(isNavKey("")).toBe(false);
  });
});

describe("nextIndexFor", () => {
  it("ArrowDown advances by 1", () => {
    expect(nextIndexFor("ArrowDown", 0, 5)).toBe(1);
    expect(nextIndexFor("ArrowDown", 2, 5)).toBe(3);
  });

  it("ArrowDown wraps at the end", () => {
    expect(nextIndexFor("ArrowDown", 4, 5)).toBe(0);
  });

  it("ArrowUp goes back by 1", () => {
    expect(nextIndexFor("ArrowUp", 3, 5)).toBe(2);
    expect(nextIndexFor("ArrowUp", 1, 5)).toBe(0);
  });

  it("ArrowUp wraps at the start", () => {
    expect(nextIndexFor("ArrowUp", 0, 5)).toBe(4);
  });

  it("returns the same index for non-arrow keys", () => {
    expect(nextIndexFor("Enter", 2, 5)).toBe(2);
    expect(nextIndexFor("a", 2, 5)).toBe(2);
  });

  it("returns 0 when the list is empty regardless of key", () => {
    expect(nextIndexFor("ArrowDown", 0, 0)).toBe(0);
    expect(nextIndexFor("ArrowUp", 0, 0)).toBe(0);
    expect(nextIndexFor("Enter", 5, 0)).toBe(0);
  });

  it("handles a single-element list (no movement)", () => {
    expect(nextIndexFor("ArrowDown", 0, 1)).toBe(0);
    expect(nextIndexFor("ArrowUp", 0, 1)).toBe(0);
  });
});
