import { create } from "zustand";
import type { AppError, BranchList } from "../bindings";
import { getBranches } from "../api/branches";
import { registerRefresh } from "./refreshRepo";

type BranchState = {
  repoId: string | null;
  branches: BranchList | null;
  error: AppError | null;
  refresh: (repoId: string) => Promise<void>;
};

export const useBranchStore = create<BranchState>((set, get) => ({
  repoId: null,
  branches: null,
  error: null,

  refresh: async (repoId) => {
    if (get().repoId !== repoId) set({ repoId, branches: null, error: null });
    const res = await getBranches(repoId);
    if (get().repoId !== repoId) return;
    if (res.status === "error") set({ error: res.error });
    else set({ branches: res.data, error: null });
  },
}));

registerRefresh("branches", (repoId) => useBranchStore.getState().refresh(repoId));
