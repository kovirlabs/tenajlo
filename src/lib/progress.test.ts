import { describe, expect, it } from "vitest";
import { formatProgress } from "./progress";

describe("formatProgress", () => {
  it("shows the phase and percent when known", () => {
    expect(formatProgress(null)).toBe("Starting…");
    expect(
      formatProgress({ phase: "Receiving objects", percent: 45, detail: null, remote: false }),
    ).toBe("Receiving objects 45%");
    expect(
      formatProgress({ phase: "Resolving deltas", percent: null, detail: null, remote: false }),
    ).toBe("Resolving deltas");
  });
});
