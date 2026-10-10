import { describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import type { FileDiff } from "../bindings";
import { DiffView } from "./DiffView";

describe("DiffView", () => {
  it("renders hunk headers and lines", () => {
    const diff: FileDiff = {
      type: "Text",
      hunks: [
        {
          header: "@@ -1,2 +1,2 @@",
          lines: [
            { kind: "Delete", text: "old", oldLine: 1, newLine: null, noNewline: false },
            { kind: "Add", text: "new", oldLine: null, newLine: 1, noNewline: true },
          ],
        },
      ],
    };
    render(<DiffView diff={diff} />);
    expect(screen.getByText("@@ -1,2 +1,2 @@")).toBeTruthy();
    expect(screen.getByText("old").parentElement?.className).toContain("diff-delete");
    expect(screen.getByLabelText("No newline at end of file")).toBeTruthy();
  });

  it("explains binary and oversized diffs", () => {
    render(<DiffView diff={{ type: "Binary" }} />);
    expect(screen.getByText("This binary file has changed.")).toBeTruthy();
    cleanup();
    render(<DiffView diff={{ type: "TooLarge" }} />);
    expect(screen.getByText("This diff is too large to show.")).toBeTruthy();
  });

  it("virtualizes long diffs", () => {
    const lines = Array.from({ length: 5000 }, (_, i) => ({
      kind: "Add" as const,
      text: `line ${i}`,
      oldLine: null,
      newLine: i + 1,
      noNewline: false,
    }));
    render(<DiffView diff={{ type: "Text", hunks: [{ header: "@@ -0,0 +1,5000 @@", lines }] }} />);
    expect(screen.getByText("line 0")).toBeTruthy();
    expect(screen.queryByText("line 4999")).toBeNull();
  });
});
