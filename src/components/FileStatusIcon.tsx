import type { FileStatusKind } from "../bindings";

const LABELS: Record<FileStatusKind, { short: string; label: string }> = {
  Untracked: { short: "+", label: "New file" },
  Added: { short: "+", label: "New file" },
  Modified: { short: "M", label: "Modified" },
  Deleted: { short: "−", label: "Deleted" },
  Renamed: { short: "R", label: "Renamed" },
  Copied: { short: "C", label: "Copied" },
  Conflicted: { short: "!", label: "Has conflicts" },
};

export function FileStatusIcon({ kind }: { kind: FileStatusKind }) {
  const { short, label } = LABELS[kind];
  return (
    <span className={`status-icon status-${kind.toLowerCase()}`} title={label} aria-label={label}>
      {short}
    </span>
  );
}
