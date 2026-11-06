import { useEffect } from "react";
import { useChangesStore } from "../stores/changesStore";
import { useForgejoStore } from "../stores/forgejoStore";
import { useSyncStore } from "../stores/syncStore";

/**
 * Looks up the current branch on Forgejo (its pull request) when the repository opens, the
 * branch changes, and after a fetch or push (spec §8.1, v1.4).
 */
export function useForgejoLinkRefresh(repoId: string) {
  const branch = useChangesStore((s) =>
    s.repoId === repoId ? (s.status?.branch.name ?? null) : null,
  );
  const lastFetched = useSyncStore((s) => (s.repoId === repoId ? s.state?.lastFetched : undefined));
  const ahead = useSyncStore((s) => (s.repoId === repoId ? s.state?.ahead : undefined));
  useEffect(() => {
    void useForgejoStore.getState().load(repoId, branch);
  }, [repoId, branch, lastFetched, ahead]);
}
