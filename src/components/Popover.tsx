import { useEffect, useRef, useState, type ReactNode } from "react";

type Props = {
  label: ReactNode;
  /** Accessible name for the trigger button. */
  ariaLabel: string;
  children: (close: () => void) => ReactNode;
  /** Controlled mode (e.g. opened by a keyboard shortcut). */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
};

/** A toolbar button that opens a panel. Closes on Escape or outside click. */
export function Popover({ label, ariaLabel, children, open: controlled, onOpenChange }: Props) {
  const [uncontrolled, setUncontrolled] = useState(false);
  const open = controlled ?? uncontrolled;
  const setOpen = (next: boolean | ((o: boolean) => boolean)) => {
    const value = typeof next === "function" ? next(open) : next;
    if (controlled === undefined) setUncontrolled(value);
    onOpenChange?.(value);
  };
  const ref = useRef<HTMLDivElement>(null);

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
      {open && <div className="popover-panel">{children(() => setOpen(false))}</div>}
    </div>
  );
}
