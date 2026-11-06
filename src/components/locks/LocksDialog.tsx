import { useState } from "react";
import type { LfsLock } from "../../bindings";
import { formatRelative } from "../../lib/time";
import { useLockStore } from "../../stores/lockStore";
import { useUiStore } from "../../stores/uiStore";
import { ConfirmDialog } from "../ConfirmDialog";
import { InlineError } from "../InlineError";
import { Modal } from "../Modal";
import { lockLabel } from "./LockBadge";

/** File locks (spec §8.1, v1.4): every lock, unlock / force unlock, and lock a file. */
export function LocksDialog({ repoId }: { repoId: string }) {
  const open = useUiStore((s) => s.dialog === "locks");
  const close = () => useUiStore.getState().openDialog(null);
  const { locks, error, loading, busy, load, lockPicked, unlock } = useLockStore();
  const [forcing, setForcing] = useState<LfsLock | null>(null);

  return (
    <>
      <Modal open={open && forcing === null} title="File locks" onClose={close}>
        <p className="muted">
          Lock a file before you edit it, so others know not to change it at the same time. CAD and
          PLC files can't be merged, so this avoids losing work.
        </p>
        <InlineError error={error} />
        {locks.length === 0 ? (
          <p className="pane-message muted">{loading ? "Loading…" : "No files are locked."}</p>
        ) : (
          <ul className="locks-list" aria-label="Locked files">
            {locks.map((l) => (
              <li key={l.id}>
                <span className="locks-path" title={l.path}>
                  {l.path}
                </span>
                <span className="muted">
                  {lockLabel(l)}
                  {l.lockedAt && ` · ${formatRelative(l.lockedAt)}`}
                </span>
                {l.ours ? (
                  <button
                    type="button"
                    className="secondary"
                    disabled={busy}
                    onClick={() => void unlock(l, false)}
                  >
                    Unlock
                  </button>
                ) : (
                  <button
                    type="button"
                    className="secondary"
                    disabled={busy}
                    onClick={() => setForcing(l)}
                  >
                    Force unlock…
                  </button>
                )}
              </li>
            ))}
          </ul>
        )}
        <div className="dialog-actions">
          <button
            type="button"
            className="secondary"
            disabled={loading}
            onClick={() => void load(repoId, false)}
          >
            Refresh
          </button>
          <button
            type="button"
            className="secondary"
            disabled={busy}
            onClick={() => void lockPicked()}
          >
            Lock a file…
          </button>
          <button type="button" onClick={close}>
            Done
          </button>
        </div>
      </Modal>
      <ConfirmDialog
        open={forcing !== null}
        title="Force unlock?"
        confirmLabel="Force unlock"
        onCancel={() => setForcing(null)}
        onConfirm={() => {
          const lock = forcing;
          setForcing(null);
          if (lock) void unlock(lock, true);
        }}
      >
        <p>
          <code>{forcing?.path}</code> is locked by {forcing?.owner || "someone else"}. Only force
          unlock if they've finished or left it locked by mistake: if they're still editing it, one
          of you may lose work.
        </p>
      </ConfirmDialog>
    </>
  );
}
