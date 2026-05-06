import { describe, expect, it } from "vitest";
import { filterByProject, filterKeys } from "./filter";
import type { KeyDto } from "./types";

const k = (overrides: Partial<KeyDto>): KeyDto => ({
  name: "key",
  env_var: "KEY_API_KEY",
  tags: [],
  added_at: "2026-05-06T00:00:00Z",
  updated_at: "2026-05-06T00:00:00Z",
  ...overrides,
});

const fixtures: KeyDto[] = [
  k({ name: "stripe", env_var: "STRIPE_API_KEY", tags: ["billing", "prod"] }),
  k({
    name: "stripe-test",
    env_var: "STRIPE_TEST_KEY",
    tags: ["billing", "test"],
  }),
  k({
    name: "anthropic",
    env_var: "ANTHROPIC_API_KEY",
    tags: ["ai"],
    note: "claude code",
  }),
  k({
    name: "openai",
    env_var: "OPENAI_API_KEY",
    tags: ["ai", "project:dahouse"],
  }),
  k({ name: "telnyx", env_var: "TELNYX_API_KEY", tags: ["project:aviosphere"] }),
];

describe("filterKeys", () => {
  it("returns all keys when query is empty", () => {
    expect(filterKeys(fixtures, "")).toEqual(fixtures);
  });

  it("returns all keys when query is whitespace only", () => {
    expect(filterKeys(fixtures, "   ")).toEqual(fixtures);
  });

  it("matches on name (case-insensitive)", () => {
    const out = filterKeys(fixtures, "STRIPE");
    expect(out.map((k) => k.name)).toEqual(["stripe", "stripe-test"]);
  });

  it("matches as substring, not fuzzy", () => {
    // "strpe" should NOT match "stripe" — fuzzy is intentionally off.
    expect(filterKeys(fixtures, "strpe")).toHaveLength(0);
  });

  it("matches on env_var", () => {
    const out = filterKeys(fixtures, "TELNYX");
    expect(out.map((k) => k.name)).toEqual(["telnyx"]);
  });

  it("matches on note", () => {
    const out = filterKeys(fixtures, "claude");
    expect(out.map((k) => k.name)).toEqual(["anthropic"]);
  });

  it("matches on tags", () => {
    const out = filterKeys(fixtures, "billing");
    expect(out.map((k) => k.name)).toEqual(["stripe", "stripe-test"]);
  });

  it("trims the query before matching", () => {
    expect(filterKeys(fixtures, "  stripe  ")).toHaveLength(2);
  });

  it("returns empty array when no match", () => {
    expect(filterKeys(fixtures, "nope")).toEqual([]);
  });
});

describe("filterByProject", () => {
  it("returns all keys when no project is selected", () => {
    expect(filterByProject(fixtures, null)).toEqual(fixtures);
  });

  it("matches keys with the project:<name> tag exactly", () => {
    const out = filterByProject(fixtures, "dahouse");
    expect(out.map((k) => k.name)).toEqual(["openai"]);
  });

  it("does not match keys with different projects", () => {
    const out = filterByProject(fixtures, "aviosphere");
    expect(out.map((k) => k.name)).toEqual(["telnyx"]);
  });

  it("matches case-sensitively (project tags are stored as-is)", () => {
    // The convention in the index is lowercase project names, but filter
    // honors what's actually in the tags. If the user has `project:Dahouse`,
    // selecting `dahouse` won't match — that mirrors CLI `--tag` semantics.
    expect(filterByProject(fixtures, "Dahouse")).toEqual([]);
  });

  it("returns empty array for unknown project", () => {
    expect(filterByProject(fixtures, "nonexistent")).toEqual([]);
  });

  it("ignores keys without tags array (defensive)", () => {
    const noTags: KeyDto[] = [k({ name: "x", tags: undefined as any })];
    expect(filterByProject(noTags, "any")).toEqual([]);
  });
});

describe("filterKeys + filterByProject composition", () => {
  it("project narrows then search filters within", () => {
    const projectScoped = filterByProject(fixtures, "dahouse");
    const final = filterKeys(projectScoped, "ai");
    expect(final.map((k) => k.name)).toEqual(["openai"]);
  });

  it("returns empty when search matches nothing in the selected project", () => {
    const projectScoped = filterByProject(fixtures, "dahouse");
    const final = filterKeys(projectScoped, "stripe");
    expect(final).toEqual([]);
  });
});
