import { create } from "zustand";
import type { AppError, FileDiff } from "../bindings";
import type { Result } from "../api/result";

type DiffState = {
  /** Identifies what `diff` belongs to, e.g. `work:<repo>:<path>`. */
  key: string | null;
  diff: FileDiff | null;
  error: AppError | null;
  load: (key: string, fetch: () => Promise<Result<FileDiff>>) => Promise<void>;
  clear: () => void;
};

export const useDiffStore = create<DiffState>((set, get) => ({
  key: null,
  diff: null,
  error: null,

  load: async (key, fetch) => {
    if (get().key !== key) set({ key, diff: null, error: null });
    const res = await fetch();
    // A newer selection may have replaced this one while we waited.
    if (get().key !== key) return;
    if (res.status === "error") set({ error: res.error, diff: null });
    else set({ diff: res.data, error: null });
  },

  clear: () => set({ key: null, diff: null, error: null }),
}));
