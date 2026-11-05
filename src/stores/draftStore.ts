import { create } from "zustand";

/** An unfinished commit message. `summaryTouched`: the user typed a summary, so a merge's
 *  prefilled summary no longer replaces it. */
export type Draft = { summary: string; summaryTouched: boolean; description: string };

const EMPTY: Draft = { summary: "", summaryTouched: false, description: "" };

type DraftState = {
  /** By repository id. Kept for this session, so switching repositories doesn't lose it. */
  drafts: Record<string, Draft>;
  update: (repoId: string, patch: Partial<Draft>) => void;
  clear: (repoId: string) => void;
};

export const useDraftStore = create<DraftState>((set) => ({
  drafts: {},
  update: (repoId, patch) =>
    set((s) => ({
      drafts: { ...s.drafts, [repoId]: { ...(s.drafts[repoId] ?? EMPTY), ...patch } },
    })),
  clear: (repoId) =>
    set((s) => {
      const rest = Object.fromEntries(Object.entries(s.drafts).filter(([id]) => id !== repoId));
      return { drafts: rest };
    }),
}));

/** The draft for `repoId` (empty if none). */
export function useDraft(repoId: string): Draft {
  return useDraftStore((s) => s.drafts[repoId] ?? EMPTY);
}
