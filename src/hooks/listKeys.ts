import type { KeyboardEvent } from "react";

type Options = {
  count: number;
  /** Index of the selected row, or -1. */
  active: number;
  /** Rows moved by Page Up / Page Down. */
  pageSize?: number;
  onMove: (index: number) => void;
  /** Space on the list itself (not on a checkbox inside a row). */
  onToggle?: (index: number) => void;
  /** The Menu key or Shift+F10: open the row's right-click menu. */
  onMenu?: (index: number) => void;
};

/**
 * Keyboard handling for a single-select list (`role="listbox"`): arrows, Home/End and
 * Page Up/Down move the selection; Space and the menu keys act on the selected row.
 */
export function listKeyHandler({
  count,
  active,
  pageSize = 10,
  onMove,
  onToggle,
  onMenu,
}: Options) {
  return (e: KeyboardEvent) => {
    if (count === 0 || e.altKey || e.ctrlKey || e.metaKey) return;
    const last = count - 1;
    const move = (to: number) => {
      e.preventDefault();
      const next = Math.max(0, Math.min(last, to));
      if (next !== active) onMove(next);
    };
    switch (e.key) {
      case "ArrowDown":
        return move(active < 0 ? 0 : active + 1);
      case "ArrowUp":
        return move(active < 0 ? 0 : active - 1);
      case "Home":
        return move(0);
      case "End":
        return move(last);
      case "PageDown":
        return move(active + pageSize);
      case "PageUp":
        return move(active - pageSize);
    }
    if (active < 0) return;
    // A checkbox inside the row handles its own Space.
    if (e.key === " " && onToggle && e.target === e.currentTarget) {
      e.preventDefault();
      onToggle(active);
    } else if (onMenu && (e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey))) {
      e.preventDefault();
      onMenu(active);
    }
  };
}

/** Where to open a keyboard-triggered context menu: just below the start of the row. */
export function menuPositionFor(id: string): { x: number; y: number } {
  const rect = document.getElementById(id)?.getBoundingClientRect();
  return rect ? { x: rect.left + 24, y: rect.bottom } : { x: 24, y: 24 };
}
