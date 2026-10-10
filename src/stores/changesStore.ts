import { create } from "zustand";
import type { AppError, SavedChanges, WorkingDirectoryStatus } from "../bindings";
import { getStatus } from "../api/status";
import { setStaged } from "../api/changes";
import { getSavedChanges } from "../api/branches";
import type { Result } from "../api/result";
import { refreshRepo, registerRefresh } from "./refreshRepo";
import { useUiStore } from "./uiStore";

export type MutateOptions = {
  /** Errors this returns true for are left to the caller instead of the error dialog. */
  quietError?: (error: AppError) => boolean;
};

type ChangesState = {
  repoId: string | null;
  status: WorkingDirectoryStatus | null;
  error: AppError | null;
  selectedPath: string | null;
  /** Changes Tenajlo saved when the user last left this branch. */
  saved: SavedChanges | null;
  /** A mutating operation is running; disable controls. */
  busy: boolean;
  refresh: (repoId: string) => Promise<void>;
  selectFile: (path: string | null) => void;
  /**
   * Runs a mutating operation on the current repository, then refreshes everything shown
   * for it. Errors go to the error dialog. Resolves to the operation's result, or `null`
   * if it didn't run (no repository, or another operation is running).
   */
  mutate: <T>(
    op: (repoId: string) => Promise<Result<T>>,
    options?: MutateOptions,
  ) => Promise<Result<T> | null>;
  setStaged: (paths: string[], staged: boolean) => Promise<void>;
  reset: () => void;
};

const initial = {
  repoId: null,
  status: null,
  error: null,
  selectedPath: null,
  saved: null,
  busy: false,
};

export const useChangesStore = create<ChangesState>((set, get) => ({
  ...initial,

  refresh: async (repoId) => {
    if (get().repoId !== repoId) set({ ...initial, repoId });
    const [res, saved] = await Promise.all([getStatus(repoId), getSavedChanges(repoId)]);
    // Ignore results for a repository the user has since switched away from.
    if (get().repoId !== repoId) return;
    if (res.status === "error") return set({ error: res.error });
    const files = res.data.files;
    const selected = get().selectedPath;
    const keep = selected !== null && files.some((f) => f.path === selected);
    set({
      status: res.data,
      saved: saved.status === "ok" ? saved.data : null,
      error: null,
      selectedPath: keep ? selected : (files[0]?.path ?? null),
    });
  },

  selectFile: (path) => set({ selectedPath: path }),

  mutate: async (op, options) => {
    const repoId = get().repoId;
    if (!repoId || get().busy) return null;
    set({ busy: true });
    try {
      const res = await op(repoId);
      if (res.status === "error" && !options?.quietError?.(res.error)) {
        useUiStore.getState().showError(res.error);
      }
      return res;
    } finally {
      set({ busy: false });
      await refreshRepo(repoId);
    }
  },

  setStaged: async (paths, staged) => {
    if (paths.length === 0) return;
    await get().mutate((repoId) => setStaged(repoId, paths, staged));
  },
  reset: () => set(initial),
}));

registerRefresh("changes", (repoId) => useChangesStore.getState().refresh(repoId));
