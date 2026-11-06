import { describe, expect, it, vi } from "vitest";
import type { KeyboardEvent } from "react";
import { listKeyHandler } from "./listKeys";

const key = (k: string, extra: Partial<KeyboardEvent> = {}) => {
  const target = {};
  return {
    key: k,
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    target,
    currentTarget: target,
    preventDefault: vi.fn(),
    ...extra,
  } as unknown as KeyboardEvent;
};

describe("listKeyHandler", () => {
  it("moves within bounds", () => {
    const onMove = vi.fn();
    const h = (active: number) => listKeyHandler({ count: 5, active, pageSize: 3, onMove });
    h(-1)(key("ArrowDown"));
    h(4)(key("ArrowDown")); // already last: no move
    h(2)(key("ArrowUp"));
    h(2)(key("End"));
    h(2)(key("Home"));
    h(0)(key("PageDown"));
    h(1)(key("PageUp"));
    expect(onMove.mock.calls.map((c) => c[0])).toEqual([0, 1, 4, 0, 3, 0]);
  });

  it("toggles on Space only when the list itself has focus", () => {
    const onToggle = vi.fn();
    const h = listKeyHandler({ count: 2, active: 1, onMove: vi.fn(), onToggle });
    h(key(" "));
    h(key(" ", { target: {} as EventTarget }));
    expect(onToggle).toHaveBeenCalledTimes(1);
    expect(onToggle).toHaveBeenCalledWith(1);
  });

  it("opens the menu from the Menu key and Shift+F10", () => {
    const onMenu = vi.fn();
    const h = listKeyHandler({ count: 2, active: 0, onMove: vi.fn(), onMenu });
    h(key("ContextMenu"));
    h(key("F10", { shiftKey: true }));
    h(key("F10"));
    expect(onMenu).toHaveBeenCalledTimes(2);
  });

  it("ignores shortcuts with modifiers", () => {
    const onMove = vi.fn();
    listKeyHandler({ count: 3, active: 0, onMove })(key("ArrowDown", { ctrlKey: true }));
    expect(onMove).not.toHaveBeenCalled();
  });
});
