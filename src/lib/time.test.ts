import { describe, expect, it } from "vitest";
import { formatRelative } from "./time";

describe("formatRelative", () => {
  const now = new Date("2026-10-06T12:00:00Z");

  it("formats past times", () => {
    expect(formatRelative("2026-10-06T11:59:30Z", now)).toBe("just now");
    expect(formatRelative("2026-10-06T09:00:00Z", now)).toMatch(/3 hours ago/);
    expect(formatRelative("2026-10-05T08:00:00-04:00", now)).toMatch(/yesterday|1 day ago/);
  });

  it("handles invalid input", () => {
    expect(formatRelative("nope", now)).toBe("");
  });
});
