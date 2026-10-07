import { create } from "zustand";
import type { AppError, Progress, RemoteRepository, Repository } from "../bindings";
import * as api from "../api/clone";
import { cancelOperation } from "../api/sync";
import type { Result } from "../api/result";

/** Repository lists are reused for a few minutes; the Refresh button forces a reload. */
const CACHE_MS = 5 * 60_000;

type RepoList = { repos: RemoteRepository[]; fetchedAt: number };

type CloneStoreState = {
  lists: Record<string, RepoList>;
  loadingAccount: string | null;
  listError: AppError | null;
  running: { opId: string; progress: Progress | null } | null;
  loadRepos: (accountId: string, force?: boolean) => Promise<void>;
  clone: (url: string, path: string) => Promise<Result<Repository>>;
  cancel: () => void;
  onProgress: (opId: string, progress: Progress) => void;
};

export const useCloneStore = create<CloneStoreState>((set, get) => ({
  lists: {},
  loadingAccount: null,
  listError: null,
  running: null,

  loadRepos: async (accountId, force = false) => {
    const cached = get().lists[accountId];
    if (!force && cached && Date.now() - cached.fetchedAt < CACHE_MS) {
      set({ listError: null });
      return;
    }
    set({ loadingAccount: accountId, listError: null });
    const res = await api.listForgejoRepositories(accountId);
    if (get().loadingAccount !== accountId) return;
    if (res.status === "error") {
      set({ loadingAccount: null, listError: res.error });
      return;
    }
    set((s) => ({
      loadingAccount: null,
      lists: { ...s.lists, [accountId]: { repos: res.data, fetchedAt: Date.now() } },
    }));
  },

  clone: async (url, path) => {
    const opId = crypto.randomUUID();
    set({ running: { opId, progress: null } });
    const res = await api.cloneRepository(opId, url, path);
    if (get().running?.opId === opId) set({ running: null });
    return res;
  },

  cancel: () => {
    const running = get().running;
    if (running) void cancelOperation(running.opId);
  },

  onProgress: (opId, progress) => {
    const running = get().running;
    if (running?.opId === opId) set({ running: { opId, progress } });
  },
}));
