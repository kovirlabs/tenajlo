import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";

type Props<T> = {
  items: readonly T[];
  rowHeight: number;
  renderRow: (item: T, index: number) => ReactNode;
  className?: string;
  /** Extra rows rendered above and below the viewport. */
  overscan?: number;
  ariaLabel?: string;
  /** Called when the last rendered row is within `overscan` of the end. */
  onNearEnd?: () => void;
};

/** Fixed-row-height virtualized list. Renders only the rows in view. */
export function VirtualList<T>({
  items,
  rowHeight,
  renderRow,
  className,
  overscan = 10,
  ariaLabel,
  onNearEnd,
}: Props<T>) {
  const ref = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [height, setHeight] = useState(600);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    setHeight(el.clientHeight || 600);
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => setHeight(el.clientHeight || 600));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const first = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan);
  const last = Math.min(items.length, Math.ceil((scrollTop + height) / rowHeight) + overscan);

  const nearEnd = items.length > 0 && last >= items.length;
  useEffect(() => {
    if (nearEnd) onNearEnd?.();
  }, [nearEnd, items.length, onNearEnd]);

  const rows: ReactNode[] = [];
  for (let i = first; i < last; i++) {
    rows.push(
      <div key={i} className="vrow" style={{ top: i * rowHeight, height: rowHeight }}>
        {renderRow(items[i] as T, i)}
      </div>,
    );
  }

  return (
    <div
      ref={ref}
      className={`vlist ${className ?? ""}`}
      onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
      aria-label={ariaLabel}
    >
      <div className="vlist-inner" style={{ height: items.length * rowHeight }}>
        {rows}
      </div>
    </div>
  );
}
