import { create } from "zustand";
import type { AppError, WorkingDirectoryStatus } from "../bindings";
import { getStatus } from "../api/status";

type ChangesState = {
  repoId: string | null;
  status: WorkingDirectoryStatus | null;
  error: AppError | null;
  selectedPath: string | null;
  refresh: (repoId: string) => Promise<void>;
  selectFile: (path: string | null) => void;
  reset: () => void;
};

const initial = { repoId: null, status: null, error: null, selectedPath: null };

export const useChangesStore = create<ChangesState>((set, get) => ({
  ...initial,

  refresh: async (repoId) => {
    if (get().repoId !== repoId) set({ ...initial, repoId });
    const res = await getStatus(repoId);
    // Ignore results for a repository the user has since switched away from.
    if (get().repoId !== repoId) return;
    if (res.status === "error") return set({ error: res.error });
    const files = res.data.files;
    const selected = get().selectedPath;
    const keep = selected !== null && files.some((f) => f.path === selected);
    set({
      status: res.data,
      error: null,
      selectedPath: keep ? selected : (files[0]?.path ?? null),
    });
  },

  selectFile: (path) => set({ selectedPath: path }),
  reset: () => set(initial),
}));
