import { useMemo, useRef } from "react";
import type { DiffLine, FileDiff, LineRef, LineStaging, StagedState } from "../bindings";
import { StageCheckbox } from "./StageCheckbox";
import { VirtualList } from "./VirtualList";

type Row =
  | { type: "hunk"; header: string; hunk: number }
  | { type: "line"; line: DiffLine; hunk: number; index: number };

/** Lets the user include single lines in the next commit. */
export type LineStagingProps = {
  lines: LineStaging;
  disabled: boolean;
  onChange: (lines: LineRef[], staged: boolean) => void;
};

const MARKER = { Add: "+", Delete: "-", Context: " " } as const;
const ROW_HEIGHT = 20;

/** Unified diff renderer, virtualized for large files. */
export function DiffView({
  diff,
  staging,
}: {
  diff: FileDiff;
  staging?: LineStagingProps | undefined;
}) {
  const rows = useMemo<Row[]>(() => {
    if (diff.type !== "Text") return [];
    return diff.hunks.flatMap((h, hunk) => [
      { type: "hunk" as const, header: h.header, hunk },
      ...h.lines.map((line, index) => ({ type: "line" as const, line, hunk, index })),
    ]);
  }, [diff]);
  // Row of the last line clicked, for Shift+click ranges.
  const anchor = useRef<number | null>(null);

  switch (diff.type) {
    case "Binary":
      return <p className="pane-message muted">This binary file has changed.</p>;
    case "TooLarge":
      return <p className="pane-message muted">This diff is too large to show.</p>;
    case "Unchanged":
      return <p className="pane-message muted">No content changes in this file.</p>;
    case "Text":
      break;
  }

  const isStaged = (hunk: number, index: number) => staging?.lines.staged[hunk]?.[index] ?? false;

  const hunkState = (hunk: number): StagedState => {
    const flags = changeRows(rows, hunk).map((r) => isStaged(r.hunk, r.index));
    if (flags.every(Boolean)) return "Full";
    return flags.some(Boolean) ? "Partial" : "None";
  };

  const toggleLine = (rowIndex: number, row: Extract<Row, { type: "line" }>, range: boolean) => {
    if (!staging) return;
    const stage = !isStaged(row.hunk, row.index);
    const from = range && anchor.current !== null ? Math.min(anchor.current, rowIndex) : rowIndex;
    const to = range && anchor.current !== null ? Math.max(anchor.current, rowIndex) : rowIndex;
    anchor.current = rowIndex;
    const refs = rows
      .slice(from, to + 1)
      .filter(isChangeRow)
      .map((r) => ({ hunk: r.hunk, line: r.index }));
    staging.onChange(refs, stage);
  };

  return (
    <VirtualList
      className="diff"
      ariaLabel="Diff"
      items={rows}
      rowHeight={ROW_HEIGHT}
      renderRow={(row, rowIndex) =>
        row.type === "hunk" ? (
          <div className={staging ? "diff-row diff-hunk diff-stageable" : "diff-row diff-hunk"}>
            {staging && (
              <span className="diff-gutter">
                <StageCheckbox
                  state={hunkState(row.hunk)}
                  label="Include this block of changes"
                  disabled={staging.disabled}
                  onChange={(stage) =>
                    staging.onChange(
                      changeRows(rows, row.hunk).map((r) => ({ hunk: r.hunk, line: r.index })),
                      stage,
                    )
                  }
                />
              </span>
            )}
            <span className="diff-hunk-text">{row.header}</span>
          </div>
        ) : (
          <DiffLineRow line={row.line}>
            {staging &&
              (row.line.kind === "Context" ? (
                <span className="diff-gutter" />
              ) : (
                <span className="diff-gutter">
                  <LineToggle
                    line={row.line}
                    staged={isStaged(row.hunk, row.index)}
                    disabled={staging.disabled}
                    onToggle={(range) => toggleLine(rowIndex, row, range)}
                  />
                </span>
              ))}
          </DiffLineRow>
        )
      }
    />
  );
}

function isChangeRow(row: Row): row is Extract<Row, { type: "line" }> {
  return row.type === "line" && row.line.kind !== "Context";
}

/** The added and removed lines of one hunk. */
function changeRows(rows: Row[], hunk: number) {
  return rows.filter(isChangeRow).filter((r) => r.hunk === hunk);
}

type ToggleProps = {
  line: DiffLine;
  staged: boolean;
  disabled: boolean;
  /** `range` is true for Shift+click: apply to every line since the last one clicked. */
  onToggle: (range: boolean) => void;
};

function LineToggle({ line, staged, disabled, onToggle }: ToggleProps) {
  const what = line.kind === "Add" ? "added" : "removed";
  const number = line.newLine ?? line.oldLine ?? "";
  // A real checkbox, like the file list's. Toggling happens in onClick, which knows about
  // Shift; the box stays controlled by `staged` until the refreshed diff arrives.
  return (
    <input
      type="checkbox"
      aria-label={`Include ${what} line ${number}`}
      checked={staged}
      disabled={disabled}
      onChange={() => {}}
      onClick={(e) => onToggle(e.shiftKey)}
    />
  );
}

function DiffLineRow({ line, children }: { line: DiffLine; children?: React.ReactNode }) {
  return (
    <div className={`diff-row diff-${line.kind.toLowerCase()}`}>
      {children}
      <span className="diff-num">{line.oldLine ?? ""}</span>
      <span className="diff-num">{line.newLine ?? ""}</span>
      <span className="diff-marker">{MARKER[line.kind]}</span>
      <span className="diff-text">{line.text}</span>
      {line.noNewline && (
        <span
          className="diff-eof"
          title="No newline at end of file"
          aria-label="No newline at end of file"
        >
          ⏎̸
        </span>
      )}
    </div>
  );
}
