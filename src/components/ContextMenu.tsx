import { useEffect, useEffectEvent, useLayoutEffect, useRef } from "react";
import { clampShift } from "../lib/viewport";

export type MenuItem = { label: string; onSelect: () => void; disabled?: boolean };

type Props = { x: number; y: number; items: MenuItem[]; onClose: () => void };

/**
 * Right-click menu at a screen position, nudged to stay inside the window.
 * Closes on outside click, Escape, Tab, or selection. Arrow keys, Home and End move between
 * items (ported from polish/after-ui-test d00b7a6).
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

  // Callers pass an inline onClose, so read it through an effect event: the listeners and
  // the initial focus must not reset each time the parent re-renders.
  const close = useEffectEvent(onClose);

  useEffect(() => {
    // Give focus back to whatever had it (the list row, for keyboard users) on close.
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    // The ref is already cleared when the cleanup runs.
    const menu = ref.current;
    menu?.querySelector("button")?.focus();
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) close();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" || e.key === "Tab") {
        e.preventDefault();
        close();
        return;
      }
      const buttons = [
        ...(menu?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []),
      ];
      if (buttons.length === 0) return;
      const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
      const next = {
        ArrowDown: (at + 1) % buttons.length,
        ArrowUp: (at - 1 + buttons.length) % buttons.length,
        Home: 0,
        End: buttons.length - 1,
      }[e.key];
      if (next === undefined) return;
      e.preventDefault();
      buttons[next]?.focus();
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
      // Unless something else took focus meanwhile, such as a dialog the chosen item opened.
      const now = document.activeElement;
      const free = now === null || now === document.body || Boolean(menu?.contains(now));
      if (free && opener?.isConnected) opener.focus();
    };
  }, []);

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
