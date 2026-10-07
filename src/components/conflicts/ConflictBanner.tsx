import { useState } from "react";
import type { ConflictedFile, OperationKind } from "../../bindings";
import { openRepoFile } from "../../api/merge";
import { useConflictStore } from "../../stores/conflictStore";
import { useUiStore } from "../../stores/uiStore";
import { ConfirmDialog } from "../ConfirmDialog";

const NAMES: Record<OperationKind, string> = {
  Merge: "merge",
  Rebase: "rebase",
  CherryPick: "cherry-pick",
  Revert: "revert",
};

/** Shown while a merge (or a rebase started elsewhere) is in progress (spec §8.1). */
export function ConflictBanner({ repoId }: { repoId: string }) {
  const state = useConflictStore((s) => (s.repoId === repoId ? s.state : null));
  const markResolved = useConflictStore((s) => s.markResolved);
  const abort = useConflictStore((s) => s.abort);
  const [confirmAbort, setConfirmAbort] = useState(false);
  const [confirmMarkers, setConfirmMarkers] = useState<ConflictedFile | null>(null);
  const operation = state?.operation ?? null;
  if (!state || (!operation && state.conflicts.length === 0)) return null;

  const name = operation ? NAMES[operation] : "operation";
  let heading: string;
  if (state.conflicts.length > 0) {
    heading =
      operation === "Merge"
        ? `Resolve ${state.conflicts.length === 1 ? "this conflict" : `these ${state.conflicts.length} conflicts`} to finish merging the server's changes.`
        : "Some files have conflicts.";
  } else if (operation === "Merge") {
    heading = "All conflicts are resolved. Commit to finish the merge.";
  } else {
    heading = `A ${name} started outside Tenajlo is in progress. Finish it in a terminal, or abort it.`;
  }

  const open = async (path: string) => {
    const res = await openRepoFile(repoId, path);
    if (res.status === "error") useUiStore.getState().showError(res.error);
  };

  return (
    <section className="conflict-banner" role="region" aria-label="Conflicts">
      <p>
        <strong>{heading}</strong>
      </p>
      {state.conflicts.length > 0 && (
        <ul className="conflict-list">
          {state.conflicts.map((f) => (
            <li key={f.path}>
              <span className="conflict-path" title={f.path}>
                {f.path}
              </span>
              <span className="muted">{describe(f)}</span>
              <button type="button" className="secondary" onClick={() => void open(f.path)}>
                Open
              </button>
              <button
                type="button"
                onClick={() => (f.markers ? setConfirmMarkers(f) : void markResolved([f.path]))}
              >
                Mark resolved
              </button>
            </li>
          ))}
        </ul>
      )}
      {operation && (
        <button type="button" className="secondary" onClick={() => setConfirmAbort(true)}>
          Abort {name}
        </button>
      )}
      <ConfirmDialog
        open={confirmAbort}
        title={`Abort the ${name}?`}
        confirmLabel={`Abort ${name}`}
        onCancel={() => setConfirmAbort(false)}
        onConfirm={() => {
          setConfirmAbort(false);
          void abort();
        }}
      >
        <p>
          Your branch goes back to how it was before the {name} started. Any conflicts you've
          already resolved will be lost.
        </p>
      </ConfirmDialog>
      <ConfirmDialog
        open={confirmMarkers !== null}
        title="This file still has conflict markers"
        confirmLabel="Mark resolved anyway"
        onCancel={() => setConfirmMarkers(null)}
        onConfirm={() => {
          const file = confirmMarkers;
          setConfirmMarkers(null);
          if (file) void markResolved([file.path]);
        }}
      >
        <p>
          <code>{confirmMarkers?.path}</code> still contains lines starting with{" "}
          <code>&lt;&lt;&lt;&lt;&lt;&lt;&lt;</code>. Edit the file to keep the right version of each
          conflicting part first, unless those lines belong there.
        </p>
      </ConfirmDialog>
    </section>
  );
}

function describe(f: ConflictedFile): string {
  if (f.markers === null) return "Conflicted";
  if (f.markers === 0) return "No conflict markers left";
  return f.markers === 1 ? "1 conflict" : `${f.markers} conflicts`;
}
