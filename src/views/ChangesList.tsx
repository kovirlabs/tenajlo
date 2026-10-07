import type { FileChange, StagedState } from "../bindings";
import { FileStatusIcon } from "../components/FileStatusIcon";
import { StageCheckbox } from "../components/StageCheckbox";
import { useChangesStore } from "../stores/changesStore";
import { useFileActions } from "./FileActions";
import { SavedChangesBanner } from "./SavedChangesBanner";

/** Aggregate stage state for the "all files" checkbox. */
function overall(files: FileChange[]): StagedState {
  if (files.length > 0 && files.every((f) => f.staged === "Full")) return "Full";
  return files.some((f) => f.staged !== "None") ? "Partial" : "None";
}

export function ChangesList() {
  const status = useChangesStore((s) => s.status);
  const error = useChangesStore((s) => s.error);
  const selectedPath = useChangesStore((s) => s.selectedPath);
  const selectFile = useChangesStore((s) => s.selectFile);
  const busy = useChangesStore((s) => s.busy);
  const setStaged = useChangesStore((s) => s.setStaged);
  const actions = useFileActions();

  if (error) return <p className="pane-message">{error.message}</p>;
  if (!status) return <p className="pane-message muted">Loading…</p>;

  const files = status.files;
  const stageable = files.filter((f) => f.kind !== "Conflicted");
  return (
    <div className="changes-list">
      <SavedChangesBanner />
      <div className="list-header">
        {files.length > 0 && (
          <StageCheckbox
            state={overall(stageable)}
            label="Include all files"
            disabled={busy || stageable.length === 0}
            onChange={(stage) =>
              void setStaged(
                stageable.map((f) => f.path),
                stage,
              )
            }
          />
        )}
        <span>
          {files.length === 0
            ? "No changes"
            : `${files.length} changed ${files.length === 1 ? "file" : "files"}`}
        </span>
      </div>
      <ul role="listbox" aria-label="Changed files" className="file-list">
        {files.map((f) => (
          <FileRow
            key={f.path}
            file={f}
            selected={f.path === selectedPath}
            busy={busy}
            onSelect={() => selectFile(f.path)}
            onStage={(stage) => void setStaged([f.path], stage)}
            onContextMenu={(e) => actions.open(f, e)}
          />
        ))}
      </ul>
      {actions.ui}
    </div>
  );
}

type RowProps = {
  file: FileChange;
  selected: boolean;
  busy: boolean;
  onSelect: () => void;
  onStage: (stage: boolean) => void;
  onContextMenu: (e: React.MouseEvent) => void;
};

function FileRow({ file, selected, busy, onSelect, onStage, onContextMenu }: RowProps) {
  const title = file.oldPath ? `${file.oldPath} → ${file.path}` : file.path;
  const conflicted = file.kind === "Conflicted";
  return (
    <li
      role="option"
      aria-selected={selected}
      className={selected ? "file-row selected" : "file-row"}
      onClick={onSelect}
      onContextMenu={onContextMenu}
      title={conflicted ? `${title} — resolve the conflict first` : title}
    >
      <StageCheckbox
        state={file.staged}
        label={`Include ${file.path}`}
        disabled={busy || conflicted}
        onChange={onStage}
      />
      <span className="file-path">
        {file.path}
        {file.submodule && <span className="muted"> (submodule)</span>}
      </span>
      <FileStatusIcon kind={file.kind} />
    </li>
  );
}
