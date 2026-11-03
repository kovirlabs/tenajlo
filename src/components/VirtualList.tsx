import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";

type Props<T> = {
  items: readonly T[];
  rowHeight: number;
  /** `id` is the row's DOM id when `idPrefix` is set; put it on the row's `option`. */
  renderRow: (item: T, index: number, id: string | undefined) => ReactNode;
  className?: string;
  /** Extra rows rendered above and below the viewport. */
  overscan?: number;
  ariaLabel?: string;
  /** ARIA role of the scroll container, e.g. `listbox` when rows are `option`s. */
  role?: string;
  /** Called when the last rendered row is within `overscan` of the end. Pass a stable
   *  function: a new one each render re-fires the call. */
  onNearEnd?: () => unknown;
  /** CSS width of the scrolled content, for rows wider than the list (long diff lines).
   *  Rows are absolutely positioned, so they can't widen it themselves. */
  contentWidth?: string | undefined;
  /** Makes the list focusable and keyboard-driven (see `listKeyHandler`). Rows get ids
   *  `${idPrefix}-${index}`, and the active one is announced via aria-activedescendant. */
  idPrefix?: string;
  /** The selected row, kept scrolled into view. -1 for none. */
  activeIndex?: number;
  onKeyDown?: (e: KeyboardEvent<HTMLDivElement>) => void;
};

/** DOM id of row `index` in a list with `idPrefix`. */
export function rowId(idPrefix: string, index: number): string {
  return `${idPrefix}-${index}`;
}

/** Fixed-row-height virtualized list. Renders only the rows in view. */
export function VirtualList<T>({
  items,
  rowHeight,
  renderRow,
  className,
  overscan = 10,
  ariaLabel,
  role,
  onNearEnd,
  contentWidth,
  idPrefix,
  activeIndex = -1,
  onKeyDown,
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

  // Keep the selected row visible (keyboard moves, or a selection restored after a refresh).
  useEffect(() => {
    const el = ref.current;
    if (!el || activeIndex < 0) return;
    const top = activeIndex * rowHeight;
    if (top < el.scrollTop) el.scrollTop = top;
    else if (top + rowHeight > el.scrollTop + el.clientHeight) {
      el.scrollTop = top + rowHeight - el.clientHeight;
    }
  }, [activeIndex, rowHeight]);

  const nearEnd = items.length > 0 && last >= items.length;
  useEffect(() => {
    if (nearEnd) void onNearEnd?.();
  }, [nearEnd, items.length, onNearEnd]);

  const rows: ReactNode[] = [];
  for (let i = first; i < last; i++) {
    rows.push(
      <div key={i} className="vrow" style={{ top: i * rowHeight, height: rowHeight }}>
        {renderRow(items[i] as T, i, idPrefix && rowId(idPrefix, i))}
      </div>,
    );
  }

  return (
    <div
      ref={ref}
      className={`vlist ${className ?? ""}`}
      onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
      aria-label={ariaLabel}
      role={role}
      tabIndex={idPrefix ? 0 : undefined}
      aria-activedescendant={
        idPrefix && activeIndex >= 0 && activeIndex < items.length
          ? rowId(idPrefix, activeIndex)
          : undefined
      }
      onKeyDown={onKeyDown}
    >
      <div
        className="vlist-inner"
        style={{ height: items.length * rowHeight, ...(contentWidth && { width: contentWidth }) }}
      >
        {rows}
      </div>
    </div>
  );
}
