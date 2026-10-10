/**
 * One place that reloads everything shown for a repository. Each store that shows
 * repository state registers its refresh here, so a change made anywhere (a commit, a sync,
 * a file watcher event) reloads all of them exactly once, and stores never import each other.
 * History is not registered: it reloads when the HEAD commit changes (useRepoRefresh).
 */

type Refresh = (repoId: string) => Promise<void>;

const refreshers = new Map<string, Refresh>();

/** Registers a store's refresh under `name` (re-registering replaces it, e.g. on hot reload). */
export function registerRefresh(name: string, refresh: Refresh): void {
  refreshers.set(name, refresh);
}

/** Reloads every registered store for `repoId`. */
export async function refreshRepo(repoId: string): Promise<void> {
  await Promise.all([...refreshers.values()].map((refresh) => refresh(repoId)));
}
