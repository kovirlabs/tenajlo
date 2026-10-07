import { useEffect } from "react";
import { backgroundFetch } from "../api/sync";
import { useSettingsStore } from "../stores/settingsStore";
import { useSyncStore } from "../stores/syncStore";

/**
 * Fetches the open repository every few minutes (Settings → Repositories; spec §8.1) so the
 * sync button can say "Pull origin (↓3)". Skipped while another sync runs; failures are
 * silent — the user's own fetch will explain them.
 */
export function useBackgroundFetch(repoId: string) {
  const minutes = useSettingsStore((s) => s.settings?.backgroundFetchMinutes ?? 0);
  useEffect(() => {
    if (minutes <= 0) return;
    let busy = false;
    const timer = window.setInterval(() => {
      const sync = useSyncStore.getState();
      const canFetch = sync.repoId === repoId && sync.state?.action.type !== "NoRemote";
      if (busy || sync.running || !canFetch) return;
      busy = true;
      void backgroundFetch(repoId)
        .then(() => useSyncStore.getState().refresh(repoId))
        .finally(() => {
          busy = false;
        });
    }, minutes * 60_000);
    return () => window.clearInterval(timer);
  }, [repoId, minutes]);
}
