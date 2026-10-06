import type { FileChange } from "../bindings";
import { FileStatusIcon } from "../components/FileStatusIcon";
import { StageCheckbox } from "../components/StageCheckbox";
import { useChangesStore } from "../stores/changesStore";

export function ChangesList() {
  const status = useChangesStore((s) => s.status);
  const error = useChangesStore((s) => s.error);
  const selectedPath = useChangesStore((s) => s.selectedPath);
  const selectFile = useChangesStore((s) => s.selectFile);

  if (error) return <p className="pane-message">{error.message}</p>;
  if (!status) return <p className="pane-message muted">Loading…</p>;

  const files = status.files;
  return (
    <div className="changes-list">
      <div className="list-header">
        {files.length === 0
          ? "No changes"
          : `${files.length} changed ${files.length === 1 ? "file" : "files"}`}
      </div>
      <ul role="listbox" aria-label="Changed files" className="file-list">
        {files.map((f) => (
          <FileRow
            key={f.path}
            file={f}
            selected={f.path === selectedPath}
            onSelect={() => selectFile(f.path)}
          />
        ))}
      </ul>
    </div>
  );
}

type RowProps = { file: FileChange; selected: boolean; onSelect: () => void };

function FileRow({ file, selected, onSelect }: RowProps) {
  const title = file.oldPath ? `${file.oldPath} → ${file.path}` : file.path;
  return (
    <li
      role="option"
      aria-selected={selected}
      className={selected ? "file-row selected" : "file-row"}
      onClick={onSelect}
      title={title}
    >
      <StageCheckbox state={file.staged} label={`Include ${file.path}`} />
      <span className="file-path">
        {file.path}
        {file.submodule && <span className="muted"> (submodule)</span>}
      </span>
      <FileStatusIcon kind={file.kind} />
    </li>
  );
}
