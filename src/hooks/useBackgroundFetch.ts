import { useEffect } from "react";
import type { SyncState } from "../bindings";
import { backgroundFetch } from "../api/sync";
import { useSettingsStore } from "../stores/settingsStore";
import { useSyncStore } from "../stores/syncStore";

/** On opening a repository or returning to the window, fetch if the last fetch is older. */
export const STALE_AFTER_MS = 60_000;

function isStale(state: SyncState | null, now: number): boolean {
  return state?.lastFetched == null || now - state.lastFetched * 1000 > STALE_AFTER_MS;
}

/**
 * Fetches the open repository every few minutes (Settings → Repositories; spec §8.1) so the
 * sync button can say "Get 3 commits", and also right after opening it and when the window
 * regains focus if the last fetch is over a minute old. Turned off entirely when the interval
 * is 0. Skipped while another sync runs; failures are silent — the user's own fetch will
 * explain them.
 */
export function useBackgroundFetch(repoId: string) {
  const minutes = useSettingsStore((s) => s.settings?.backgroundFetchMinutes ?? 0);
  useEffect(() => {
    if (minutes <= 0) return;
    let busy = false;
    const fetchNow = (onlyIfStale: boolean) => {
      const sync = useSyncStore.getState();
      const canFetch =
        sync.repoId === repoId && sync.state !== null && sync.state.action.type !== "NoRemote";
      if (busy || sync.running || !canFetch) return;
      if (onlyIfStale && !isStale(sync.state, Date.now())) return;
      busy = true;
      sync.setBackgroundFetching(true);
      void backgroundFetch(repoId)
        .then(() => useSyncStore.getState().refresh(repoId))
        .finally(() => {
          busy = false;
          useSyncStore.getState().setBackgroundFetching(false);
        });
    };

    const timer = window.setInterval(() => fetchNow(false), minutes * 60_000);

    // Once this repository's sync state has loaded, fetch if it's stale.
    let checkedOpen = false;
    const checkOpen = () => {
      const sync = useSyncStore.getState();
      if (checkedOpen || sync.repoId !== repoId || sync.state === null) return;
      checkedOpen = true;
      fetchNow(true);
    };
    checkOpen();
    const unsubscribe = useSyncStore.subscribe(checkOpen);

    const onFocus = () => fetchNow(true);
    window.addEventListener("focus", onFocus);
    return () => {
      window.clearInterval(timer);
      unsubscribe();
      window.removeEventListener("focus", onFocus);
    };
  }, [repoId, minutes]);
}
