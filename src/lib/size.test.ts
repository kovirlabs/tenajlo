import { describe, expect, it } from "vitest";
import { formatSize, formatSizeChange } from "./size";

describe("formatSize", () => {
  it("uses bytes, then one decimal below 10 units", () => {
    expect(formatSize(1)).toBe("1 byte");
    expect(formatSize(812)).toBe("812 bytes");
    expect(formatSize(4096)).toBe("4.0 KB");
    expect(formatSize(3_000_000)).toBe("2.9 MB");
    expect(formatSize(52_428_800)).toBe("50 MB");
  });

  it("describes the change", () => {
    expect(formatSizeChange(4096, 6144)).toBe("+2.0 KB");
    expect(formatSizeChange(6144, 5632)).toBe("−512 bytes");
    expect(formatSizeChange(5, 5)).toBe("");
  });
});
