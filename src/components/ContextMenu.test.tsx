import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { ContextMenu, type MenuItem } from "./ContextMenu";

const items: MenuItem[] = [
  { label: "Open", onSelect: () => {} },
  { label: "Discard", onSelect: () => {} },
];

describe("ContextMenu", () => {
  it("keeps focus where the user moved it when the parent re-renders", () => {
    const { rerender } = render(<ContextMenu x={0} y={0} items={items} onClose={() => {}} />);
    expect(document.activeElement).toBe(screen.getByText("Open"));

    screen.getByText("Discard").focus();
    // A new inline onClose on every parent render, as real callers pass.
    rerender(<ContextMenu x={0} y={0} items={items} onClose={() => {}} />);
    expect(document.activeElement).toBe(screen.getByText("Discard"));
  });

  it("calls the latest onClose on Escape", () => {
    const first = vi.fn();
    const latest = vi.fn();
    const { rerender } = render(<ContextMenu x={0} y={0} items={items} onClose={first} />);
    rerender(<ContextMenu x={0} y={0} items={items} onClose={latest} />);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(first).not.toHaveBeenCalled();
    expect(latest).toHaveBeenCalledOnce();
  });
});
