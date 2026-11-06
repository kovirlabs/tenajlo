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
import { useTauriEvent } from "./useTauriEvent";

/**
 * Loads repository data and keeps it fresh: on open, on `repo-changed` events from the
 * file watcher, and on window focus (fallback where watching isn't possible).
 * History reloads when the HEAD commit changes, and when a push or publish changes what is on
 * the server (its "Not pushed" markers), which leaves HEAD where it was.
 */
export function useRepoRefresh(repoId: string) {
  const refreshHistory = useHistoryStore((s) => s.refresh);
  const branch = useChangesStore((s) => (s.repoId === repoId ? s.status?.branch : undefined));
  const tip = branch?.tip;
  const upstream = branch?.upstream;
  const ahead = branch?.ahead;

  useEffect(() => {
    const refresh = () => void refreshRepo(repoId);
    refresh();
    void watchRepository(repoId);

    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [repoId]);

  useTauriEvent(onRepoChanged, (id) => {
    if (id === repoId) void refreshRepo(repoId);
  });
  useTauriEvent(onProgress, (p) => {
    if (p.repoId === repoId) useSyncStore.getState().onProgress(p.opId, p.progress);
  });

  useEffect(() => {
    // `undefined` = status not loaded yet; `null` = no commits yet (still worth loading: empty).
    if (tip !== undefined) void refreshHistory(repoId);
  }, [repoId, tip, upstream, ahead, refreshHistory]);
}
