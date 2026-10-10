import { create } from "zustand";
import type { AppError, FileDiff, LineStaging, WorkingDiff } from "../bindings";
import type { Result } from "../api/result";

type DiffState = {
  /** Identifies what `diff` belongs to, e.g. `work:<repo>:<path>`. */
  key: string | null;
  diff: FileDiff | null;
  /** Which lines are staged, for working-directory files that support line staging. */
  lines: LineStaging | null;
  error: AppError | null;
  load: (key: string, fetch: () => Promise<Result<WorkingDiff>>) => Promise<void>;
  clear: () => void;
};

export const useDiffStore = create<DiffState>((set, get) => ({
  key: null,
  diff: null,
  lines: null,
  error: null,

  load: async (key, fetch) => {
    if (get().key !== key) set({ key, diff: null, lines: null, error: null });
    const res = await fetch();
    // A newer selection may have replaced this one while we waited.
    if (get().key !== key) return;
    if (res.status === "error") set({ error: res.error, diff: null, lines: null });
    else set({ diff: res.data.diff, lines: res.data.lines, error: null });
  },

  clear: () => set({ key: null, diff: null, lines: null, error: null }),
}));
