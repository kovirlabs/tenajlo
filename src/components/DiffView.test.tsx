import { describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { DiffLine, FileDiff } from "../bindings";
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

  describe("line staging", () => {
    const line = (kind: DiffLine["kind"], n: number): DiffLine => ({
      kind,
      text: `${kind} ${n}`,
      oldLine: kind === "Add" ? null : n,
      newLine: kind === "Delete" ? null : n,
      noNewline: false,
    });
    // Hunk 0: context, two removed, one added. Hunk 1: one added.
    const diff: FileDiff = {
      type: "Text",
      hunks: [
        {
          header: "@@ -1,3 +1,2 @@",
          lines: [line("Context", 1), line("Delete", 2), line("Delete", 3), line("Add", 2)],
        },
        { header: "@@ -9 +9,2 @@", lines: [line("Add", 10)] },
      ],
    };
    const staged = [[false, true, false, false], [false]];

    function setup() {
      const onChange = vi.fn();
      render(
        <DiffView
          diff={diff}
          staging={{ lines: { token: "t", staged }, disabled: false, onChange }}
        />,
      );
      return onChange;
    }

    it("shows which lines are staged and toggles one", () => {
      const onChange = setup();
      const removed2 = screen.getByRole("checkbox", { name: "Include removed line 2" });
      expect((removed2 as HTMLInputElement).checked).toBe(true);
      fireEvent.click(screen.getByRole("checkbox", { name: "Include added line 10" }));
      expect(onChange).toHaveBeenCalledWith([{ hunk: 1, line: 0 }], true);
      fireEvent.click(removed2);
      expect(onChange).toHaveBeenLastCalledWith([{ hunk: 0, line: 1 }], false);
      // Context lines have no checkbox.
      expect(screen.queryByRole("checkbox", { name: /line 1$/ })).toBeNull();
    });

    it("Shift+click applies to the range since the last click, across hunks", () => {
      const onChange = setup();
      fireEvent.click(screen.getByRole("checkbox", { name: "Include removed line 3" }));
      fireEvent.click(screen.getByRole("checkbox", { name: "Include added line 10" }), {
        shiftKey: true,
      });
      expect(onChange).toHaveBeenLastCalledWith(
        [
          { hunk: 0, line: 2 },
          { hunk: 0, line: 3 },
          { hunk: 1, line: 0 },
        ],
        true,
      );
    });

    it("includes a whole block from its header", () => {
      const onChange = setup();
      const blocks = screen.getAllByRole("checkbox", { name: "Include this block of changes" });
      expect((blocks[0] as HTMLInputElement).indeterminate).toBe(true);
      fireEvent.click(blocks[0] as HTMLElement);
      expect(onChange).toHaveBeenCalledWith(
        [
          { hunk: 0, line: 1 },
          { hunk: 0, line: 2 },
          { hunk: 0, line: 3 },
        ],
        true,
      );
    });

    it("has no checkboxes without staging support", () => {
      render(<DiffView diff={diff} />);
      expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
    });
  });
});
