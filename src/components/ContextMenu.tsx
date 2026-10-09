import { useEffect, useLayoutEffect, useRef } from "react";
import { clampShift } from "../lib/viewport";

export type MenuItem = { label: string; onSelect: () => void; disabled?: boolean };

type Props = { x: number; y: number; items: MenuItem[]; onClose: () => void };

/**
 * Right-click menu at a screen position, nudged to stay inside the window.
 * Closes on outside click, Escape, or selection.
 */
export function ContextMenu({ x, y, items, onClose }: Props) {
  const ref = useRef<HTMLUListElement>(null);

  useLayoutEffect(() => {
    const menu = ref.current;
    if (!menu) return;
    const { width, height } = menu.getBoundingClientRect();
    const { dx, dy } = clampShift(
      { left: x, top: y, width, height },
      window.innerWidth,
      window.innerHeight,
    );
    menu.style.left = `${x + dx}px`;
    menu.style.top = `${y + dy}px`;
  }, [x, y]);

  useEffect(() => {
    ref.current?.querySelector("button")?.focus();
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  return (
    <ul ref={ref} role="menu" className="context-menu" style={{ left: x, top: y }}>
      {items.map((item) => (
        <li key={item.label} role="none">
          <button
            type="button"
            role="menuitem"
            disabled={item.disabled}
            onClick={() => {
              onClose();
              item.onSelect();
            }}
          >
            {item.label}
          </button>
        </li>
      ))}
    </ul>
  );
}
