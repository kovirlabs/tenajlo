import { useMemo } from "react";
import type { DiffLine, FileDiff } from "../bindings";
import { VirtualList } from "./VirtualList";

type Row = { type: "hunk"; header: string } | { type: "line"; line: DiffLine };

const MARKER = { Add: "+", Delete: "-", Context: " " } as const;
const ROW_HEIGHT = 20;

/** Unified diff renderer, virtualized for large files. */
export function DiffView({ diff }: { diff: FileDiff }) {
  const rows = useMemo<Row[]>(() => {
    if (diff.type !== "Text") return [];
    return diff.hunks.flatMap((h) => [
      { type: "hunk" as const, header: h.header },
      ...h.lines.map((line) => ({ type: "line" as const, line })),
    ]);
  }, [diff]);

  switch (diff.type) {
    case "Binary":
      return <p className="pane-message muted">This binary file has changed.</p>;
    case "TooLarge":
      return <p className="pane-message muted">This diff is too large to show.</p>;
    case "Unchanged":
      return <p className="pane-message muted">No content changes in this file.</p>;
    case "Text":
      return (
        <VirtualList
          className="diff"
          ariaLabel="Diff"
          items={rows}
          rowHeight={ROW_HEIGHT}
          renderRow={(row) =>
            row.type === "hunk" ? (
              <div className="diff-row diff-hunk">{row.header}</div>
            ) : (
              <DiffLineRow line={row.line} />
            )
          }
        />
      );
  }
}

function DiffLineRow({ line }: { line: DiffLine }) {
  return (
    <div className={`diff-row diff-${line.kind.toLowerCase()}`}>
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
