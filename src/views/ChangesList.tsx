import { useId } from "react";
import type { FileChange, StagedState } from "../bindings";
import { FilePath, diffTitle } from "../components/FilePath";
import { FileStatusIcon } from "../components/FileStatusIcon";
import { StageCheckbox } from "../components/StageCheckbox";
import { LockBadge } from "../components/locks/LockBadge";
import { useLockFor } from "../stores/lockStore";
import { VirtualList, rowId } from "../components/VirtualList";
import { listKeyHandler, menuPositionFor } from "../hooks/listKeys";
import { useChangesStore } from "../stores/changesStore";
import { useFileActions } from "./FileActions";
import { SavedChangesBanner } from "./SavedChangesBanner";

/** Matches `.file-row` in styles.css. Rows are virtualized: a CAD or PLC repository can have
 *  thousands of changed files. */
const ROW_HEIGHT = 28;

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
  const refresh = useChangesStore((s) => s.refresh);
  const repoId = useChangesStore((s) => s.repoId);
  const actions = useFileActions();
  const idPrefix = useId();

  if (error) {
    return (
      <div className="pane-message">
        <p>{error.message}</p>
        {repoId && (
          <button type="button" className="secondary" onClick={() => void refresh(repoId)}>
            Try again
          </button>
        )}
      </div>
    );
  }
  if (!status) return <p className="pane-message muted">Loading…</p>;

  const files = status.files;
  const stageable = files.filter((f) => f.kind !== "Conflicted");
  const active = files.findIndex((f) => f.path === selectedPath);
  const toggle = (f: FileChange) => {
    if (!busy && f.kind !== "Conflicted") void setStaged([f.path], f.staged !== "Full");
  };
  const onKeyDown = listKeyHandler({
    count: files.length,
    active,
    onMove: (i) => {
      const f = files[i];
      if (f) selectFile(f.path);
    },
    onToggle: (i) => {
      const f = files[i];
      if (f) toggle(f);
    },
    onMenu: (i) => {
      const f = files[i];
      if (f) void actions.openAt(f, menuPositionFor(rowId(idPrefix, i)));
    },
  });
  return (
    <div className="changes-list">
      <SavedChangesBanner />
      <div className="list-header" onContextMenu={(e) => actions.open(null, e)}>
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
      <VirtualList
        role="listbox"
        ariaLabel="Changed files"
        className="file-list"
        items={files}
        rowHeight={ROW_HEIGHT}
        idPrefix={idPrefix}
        activeIndex={active}
        onKeyDown={onKeyDown}
        renderRow={(f, _i, id) => (
          <FileRow
            id={id}
            file={f}
            selected={f.path === selectedPath}
            busy={busy}
            onSelect={() => selectFile(f.path)}
            onStage={(stage) => void setStaged([f.path], stage)}
            onContextMenu={(e) => {
              // The menu is about this file: select it, as the keyboard's Menu key would.
              selectFile(f.path);
              actions.open(f, e);
            }}
          />
        )}
      />
      {actions.ui}
    </div>
  );
}

type RowProps = {
  id: string | undefined;
  file: FileChange;
  selected: boolean;
  busy: boolean;
  onSelect: () => void;
  onStage: (stage: boolean) => void;
  onContextMenu: (e: React.MouseEvent) => void;
};

function FileRow({ id, file, selected, busy, onSelect, onStage, onContextMenu }: RowProps) {
  const title = diffTitle(file.path, file.oldPath);
  const conflicted = file.kind === "Conflicted";
  const lock = useLockFor(file.path);
  return (
    <div
      id={id}
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
      <FilePath path={file.path}>
        {file.submodule && <span className="muted">&nbsp;(submodule)</span>}
      </FilePath>
      {lock && <LockBadge lock={lock} />}
      <FileStatusIcon kind={file.kind} />
    </div>
  );
}
