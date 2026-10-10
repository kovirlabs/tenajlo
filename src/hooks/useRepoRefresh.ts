import { useEffect } from "react";
import { onRepoChanged, watchRepository } from "../api/watch";
import { onProgress } from "../api/sync";
// The stores register themselves with refreshRepo when imported.
import { useChangesStore } from "../stores/changesStore";
import "../stores/branchStore";
import "../stores/conflictStore";
import { useHistoryStore } from "../stores/historyStore";
import { refreshRepo } from "../stores/refreshRepo";
import { useSyncStore } from "../stores/syncStore";

/**
 * Loads repository data and keeps it fresh: on open, on `repo-changed` events from the
 * file watcher, and on window focus (fallback where watching isn't possible).
 * History reloads only when the HEAD commit changes.
 */
export function useRepoRefresh(repoId: string) {
  const refreshHistory = useHistoryStore((s) => s.refresh);
  const tip = useChangesStore((s) => (s.repoId === repoId ? s.status?.branch.tip : undefined));

  useEffect(() => {
    const refresh = () => void refreshRepo(repoId);
    refresh();
    void watchRepository(repoId);

    const unlisteners: (() => void)[] = [];
    let disposed = false;
    const keep = (fn: () => void) => {
      if (disposed) fn();
      else unlisteners.push(fn);
    };
    void onRepoChanged((id) => {
      if (id === repoId) refresh();
    }).then(keep);
    void onProgress((p) => {
      if (p.repoId === repoId) useSyncStore.getState().onProgress(p.opId, p.progress);
    }).then(keep);
    window.addEventListener("focus", refresh);
    return () => {
      disposed = true;
      unlisteners.forEach((fn) => fn());
      window.removeEventListener("focus", refresh);
    };
  }, [repoId]);

  useEffect(() => {
    // `undefined` = status not loaded yet; `null` = no commits yet (still worth loading: empty).
    if (tip !== undefined) void refreshHistory(repoId);
  }, [repoId, tip, refreshHistory]);
}
