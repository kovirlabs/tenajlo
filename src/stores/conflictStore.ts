import { create } from "zustand";
import type { OperationState } from "../bindings";
import * as api from "../api/merge";
import { useChangesStore } from "./changesStore";

type ConflictStoreState = {
  repoId: string | null;
  state: OperationState | null;
  refresh: (repoId: string) => Promise<void>;
  markResolved: (paths: string[]) => Promise<void>;
  abort: () => Promise<void>;
};

export const useConflictStore = create<ConflictStoreState>((set, get) => ({
  repoId: null,
  state: null,

  refresh: async (repoId) => {
    if (get().repoId !== repoId) set({ repoId, state: null });
    const res = await api.getOperationState(repoId);
    if (get().repoId === repoId && res.status === "ok") set({ state: res.data });
  },

  markResolved: async (paths) => {
    await useChangesStore.getState().mutate((id) => api.markResolved(id, paths));
    const repoId = get().repoId;
    if (repoId) await get().refresh(repoId);
  },

  abort: async () => {
    await useChangesStore.getState().mutate((id) => api.abortOperation(id));
    const repoId = get().repoId;
    if (repoId) await get().refresh(repoId);
  },
}));

/** A merge Tenajlo can finish with a commit (as opposed to a rebase started elsewhere). */
export function useMerging(): {
  merging: boolean;
  otherOperation: boolean;
  summary: string | null;
} {
  const state = useConflictStore((s) => s.state);
  const op = state?.operation ?? null;
  return {
    merging: op === "Merge",
    otherOperation: op !== null && op !== "Merge",
    summary: state?.mergeSummary ?? null,
  };
}
