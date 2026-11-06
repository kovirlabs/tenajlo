import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { FilePath, diffTitle } from "./FilePath";

describe("FilePath", () => {
  it("splits the folder from the file name so the name can stay visible", () => {
    const { container } = render(<FilePath path="folder with spaces/ünï ✓/日本語.md" />);
    expect(container.querySelector(".file-dir")?.textContent).toBe("folder with spaces/ünï ✓/");
    expect(container.querySelector(".file-name")?.textContent).toBe("日本語.md");
  });

  it("has no folder part for a file at the top level", () => {
    const { container } = render(<FilePath path="README.md" />);
    expect(container.querySelector(".file-dir")).toBeNull();
    expect(container.querySelector(".file-name")?.textContent).toBe("README.md");
  });

  it("titles a rename with both names", () => {
    expect(diffTitle("b.txt", "a.txt")).toBe("a.txt → b.txt");
    expect(diffTitle("b.txt", null)).toBe("b.txt");
  });
});
