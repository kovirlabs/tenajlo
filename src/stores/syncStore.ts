import { create } from "zustand";
import type { Progress, SyncRequest, SyncState } from "../bindings";
import { cancelOperation, getSyncState, sync } from "../api/sync";
import { useBranchStore } from "./branchStore";
import { useChangesStore } from "./changesStore";
import { useUiStore } from "./uiStore";

type Running = { opId: string; request: SyncRequest; progress: Progress | null };

type SyncStoreState = {
  repoId: string | null;
  state: SyncState | null;
  running: Running | null;
  refresh: (repoId: string) => Promise<void>;
  run: (request: SyncRequest) => Promise<void>;
  cancel: () => void;
  onProgress: (opId: string, progress: Progress) => void;
};

export const useSyncStore = create<SyncStoreState>((set, get) => ({
  repoId: null,
  state: null,
  running: null,

  refresh: async (repoId) => {
    if (get().repoId !== repoId) set({ repoId, state: null, running: null });
    const res = await getSyncState(repoId);
    if (get().repoId === repoId && res.status === "ok") set({ state: res.data });
  },

  run: async (request) => {
    const repoId = get().repoId;
    if (!repoId || get().running) return;
    const opId = crypto.randomUUID();
    set({ running: { opId, request, progress: null } });
    const res = await sync(repoId, opId, request);
    if (get().running?.opId === opId) set({ running: null });
    // A cancelled operation needs no explanation.
    if (res.status === "error" && res.error.kind !== "GitCancelled") {
      useUiStore.getState().showError(res.error);
    }
    await Promise.all([
      get().refresh(repoId),
      useChangesStore.getState().refresh(repoId),
      useBranchStore.getState().refresh(repoId),
    ]);
  },

  cancel: () => {
    const opId = get().running?.opId;
    if (opId) void cancelOperation(opId);
  },

  onProgress: (opId, progress) => {
    const running = get().running;
    if (running?.opId === opId) set({ running: { ...running, progress } });
  },
}));
