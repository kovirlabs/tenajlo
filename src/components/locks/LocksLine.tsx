import { useLockStore } from "../../stores/lockStore";
import { useUiStore } from "../../stores/uiStore";

/** Sidebar line for repositories with LFS locks: how many files are locked, and Manage. */
export function LocksLine({ repoId }: { repoId: string }) {
  const available = useLockStore((s) => s.repoId === repoId && s.available);
  const locks = useLockStore((s) => s.locks);
  if (!available) return null;
  const mine = locks.filter((l) => l.ours).length;
  const summary =
    locks.length === 0
      ? "No files locked"
      : `${locks.length} ${locks.length === 1 ? "file" : "files"} locked` +
        (mine > 0 ? ` (${mine} by you)` : "");
  return (
    <div className="locks-line">
      <span>{summary}</span>
      <button
        type="button"
        className="link"
        onClick={() => useUiStore.getState().openDialog("locks")}
      >
        Manage
      </button>
    </div>
  );
}
