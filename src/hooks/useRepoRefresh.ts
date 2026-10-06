import { useEffect } from "react";
import { onRepoChanged, watchRepository } from "../api/watch";
import { useBranchStore } from "../stores/branchStore";
import { useChangesStore } from "../stores/changesStore";
import { useHistoryStore } from "../stores/historyStore";

/**
 * Loads repository data and keeps it fresh: on open, on `repo-changed` events from the
 * file watcher, and on window focus (fallback where watching isn't possible).
 * History reloads only when the HEAD commit changes.
 */
export function useRepoRefresh(repoId: string) {
  const refreshChanges = useChangesStore((s) => s.refresh);
  const refreshBranches = useBranchStore((s) => s.refresh);
  const refreshHistory = useHistoryStore((s) => s.refresh);
  const tip = useChangesStore((s) => (s.repoId === repoId ? s.status?.branch.tip : undefined));

  useEffect(() => {
    const refresh = () => {
      void refreshChanges(repoId);
      void refreshBranches(repoId);
    };
    refresh();
    void watchRepository(repoId);

    let unlisten: (() => void) | null = null;
    let disposed = false;
    void onRepoChanged((id) => {
      if (id === repoId) refresh();
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    window.addEventListener("focus", refresh);
    return () => {
      disposed = true;
      unlisten?.();
      window.removeEventListener("focus", refresh);
    };
  }, [repoId, refreshChanges, refreshBranches]);

  useEffect(() => {
    // `undefined` = status not loaded yet; `null` = no commits yet (still worth loading: empty).
    if (tip !== undefined) void refreshHistory(repoId);
  }, [repoId, tip, refreshHistory]);
}
