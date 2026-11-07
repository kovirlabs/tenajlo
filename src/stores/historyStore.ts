import { create } from "zustand";
import type { AppError, Commit, CommitFile } from "../bindings";
import { getCommitFiles, getHistory } from "../api/history";

export const PAGE_SIZE = 200;

type HistoryState = {
  repoId: string | null;
  commits: Commit[];
  /** No more commits to load. */
  complete: boolean;
  loading: boolean;
  error: AppError | null;
  selectedSha: string | null;
  /** The selection moves to the newest commit on refresh, until the user picks an older one. */
  followsNewest: boolean;
  files: CommitFile[] | null;
  selectedFile: CommitFile | null;
  /** Reloads from the top, keeping a commit the user picked if it still exists. */
  refresh: (repoId: string) => Promise<void>;
  loadMore: () => Promise<void>;
  selectCommit: (sha: string) => Promise<void>;
  selectFile: (file: CommitFile) => void;
};

const initial = {
  repoId: null,
  commits: [],
  complete: false,
  loading: false,
  error: null,
  selectedSha: null,
  followsNewest: true,
  files: null,
  selectedFile: null,
};

export const useHistoryStore = create<HistoryState>((set, get) => {
  const show = async (sha: string, followsNewest: boolean) => {
    const repoId = get().repoId;
    if (!repoId) return;
    set({ selectedSha: sha, followsNewest, files: null, selectedFile: null });
    const res = await getCommitFiles(repoId, sha);
    if (get().selectedSha !== sha) return;
    if (res.status === "error") return set({ error: res.error });
    set({ files: res.data, selectedFile: res.data[0] ?? null });
  };

  return {
    ...initial,

    refresh: async (repoId) => {
      if (get().repoId !== repoId) set({ ...initial, repoId });
      set({ loading: true });
      const res = await getHistory(repoId, 0, Math.max(PAGE_SIZE, get().commits.length));
      if (get().repoId !== repoId) return;
      if (res.status === "error") return set({ error: res.error, loading: false });
      const commits = res.data;
      set({ commits, complete: commits.length < PAGE_SIZE, loading: false, error: null });
      const { selectedSha, followsNewest } = get();
      const picked = followsNewest ? undefined : commits.find((c) => c.sha === selectedSha);
      const next = picked ?? commits[0];
      if (next && next.sha !== selectedSha) await show(next.sha, !picked);
      if (!next) set({ selectedSha: null, followsNewest: true, files: null, selectedFile: null });
    },

    loadMore: async () => {
      const { repoId, loading, complete, error, commits } = get();
      // After a failure, wait for the next refresh instead of retrying on every render.
      if (!repoId || loading || complete || error) return;
      const offset = commits.length;
      set({ loading: true });
      const res = await getHistory(repoId, offset, PAGE_SIZE);
      // A refresh replaced the list meanwhile; this page no longer lines up with it.
      if (get().repoId !== repoId || get().commits.length !== offset) return;
      if (res.status === "error") return set({ error: res.error, loading: false });
      set({
        commits: [...get().commits, ...res.data],
        complete: res.data.length < PAGE_SIZE,
        loading: false,
      });
    },

    selectCommit: (sha) => show(sha, sha === get().commits[0]?.sha),

    selectFile: (file) => set({ selectedFile: file }),
  };
});
