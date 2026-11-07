import { StrictMode } from "react";
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

  it("moves between items with the arrow keys, skipping disabled ones", () => {
    const three: MenuItem[] = [
      { label: "Open", onSelect: () => {} },
      { label: "Off", onSelect: () => {}, disabled: true },
      { label: "Discard", onSelect: () => {} },
    ];
    render(<ContextMenu x={0} y={0} items={three} onClose={() => {}} />);
    fireEvent.keyDown(document, { key: "ArrowDown" });
    expect(document.activeElement).toBe(screen.getByText("Discard"));
    fireEvent.keyDown(document, { key: "ArrowDown" });
    expect(document.activeElement).toBe(screen.getByText("Open"));
    fireEvent.keyDown(document, { key: "End" });
    expect(document.activeElement).toBe(screen.getByText("Discard"));
  });

  it("gives focus back on close, but not away from a dialog an item opened", () => {
    const list = document.createElement("div");
    list.tabIndex = 0;
    document.body.append(list);
    list.focus();
    const first = render(
      <StrictMode>
        <ContextMenu x={0} y={0} items={items} onClose={() => {}} />
      </StrictMode>,
    );
    expect(document.activeElement).toBe(screen.getByText("Open"));
    first.unmount();
    expect(document.activeElement).toBe(list);

    const second = render(<ContextMenu x={0} y={0} items={items} onClose={() => {}} />);
    const dialogButton = document.createElement("button");
    document.body.append(dialogButton);
    dialogButton.focus();
    second.unmount();
    expect(document.activeElement).toBe(dialogButton);
    list.remove();
    dialogButton.remove();
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
