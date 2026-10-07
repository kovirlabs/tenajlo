import { create } from "zustand";
import type { AppError, SavedChanges, WorkingDirectoryStatus } from "../bindings";
import { getStatus } from "../api/status";
import { setStaged } from "../api/changes";
import { getSavedChanges } from "../api/branches";
import { useBranchStore } from "./branchStore";
import { useUiStore } from "./uiStore";

type ChangesState = {
  repoId: string | null;
  status: WorkingDirectoryStatus | null;
  error: AppError | null;
  selectedPath: string | null;
  /** Changes Anvil saved when the user last left this branch. */
  saved: SavedChanges | null;
  /** A mutating operation is running; disable controls. */
  busy: boolean;
  refresh: (repoId: string) => Promise<void>;
  selectFile: (path: string | null) => void;
  /** Runs a mutating operation, then refreshes status. Errors go to the error dialog. */
  mutate: (
    op: (repoId: string) => Promise<{ status: "ok" } | { status: "error"; error: AppError }>,
  ) => Promise<boolean>;
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

  mutate: async (op) => {
    const repoId = get().repoId;
    if (!repoId || get().busy) return false;
    set({ busy: true });
    try {
      const res = await op(repoId);
      if (res.status === "error") useUiStore.getState().showError(res.error);
      return res.status === "ok";
    } finally {
      set({ busy: false });
      await Promise.all([get().refresh(repoId), useBranchStore.getState().refresh(repoId)]);
    }
  },

  setStaged: async (paths, staged) => {
    if (paths.length === 0) return;
    await get().mutate((repoId) => setStaged(repoId, paths, staged));
  },
  reset: () => set(initial),
}));
