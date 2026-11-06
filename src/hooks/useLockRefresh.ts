import { useEffect } from "react";
import { useLockStore } from "../stores/lockStore";
import { useSyncStore } from "../stores/syncStore";

/**
 * Loads the repository's LFS file locks when it opens and again after every fetch (the sync
 * state's "last fetched" time moves). Never prompts: the File locks dialog's Refresh does.
 */
export function useLockRefresh(repoId: string) {
  const lastFetched = useSyncStore((s) => (s.repoId === repoId ? s.state?.lastFetched : undefined));
  useEffect(() => {
    void useLockStore.getState().load(repoId, true);
  }, [repoId, lastFetched]);
}
