import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { clampShift } from "../lib/viewport";

type Props = {
  label: ReactNode;
  /** Accessible name for the trigger button. */
  ariaLabel: string;
  children: (close: () => void) => ReactNode;
  /** Controlled mode (e.g. opened by a keyboard shortcut). */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
};

/**
 * A toolbar button that opens a panel. Closes on Escape or outside click.
 * The panel shifts sideways as needed to stay inside the window.
 */
export function Popover({ label, ariaLabel, children, open: controlled, onOpenChange }: Props) {
  const [uncontrolled, setUncontrolled] = useState(false);
  const open = controlled ?? uncontrolled;
  const setOpen = (next: boolean | ((o: boolean) => boolean)) => {
    const value = typeof next === "function" ? next(open) : next;
    if (controlled === undefined) setUncontrolled(value);
    onOpenChange?.(value);
  };
  const ref = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const panel = panelRef.current;
    if (!open || !panel) return;
    const place = () => {
      panel.style.translate = "";
      const { dx } = clampShift(
        panel.getBoundingClientRect(),
        window.innerWidth,
        window.innerHeight,
      );
      panel.style.translate = dx ? `${dx}px 0` : "";
    };
    place();
    window.addEventListener("resize", place);
    return () => window.removeEventListener("resize", place);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div className="popover" ref={ref}>
      <button
        type="button"
        className="toolbar-button"
        aria-label={ariaLabel}
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        {label}
        <span aria-hidden="true"> ▾</span>
      </button>
      {open && (
        <div className="popover-panel" ref={panelRef}>
          {children(() => setOpen(false))}
        </div>
      )}
    </div>
  );
}
