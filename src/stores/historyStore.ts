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
  files: CommitFile[] | null;
  selectedFile: CommitFile | null;
  /** Reloads from the top, keeping the selection if it still exists. */
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
  files: null,
  selectedFile: null,
};

export const useHistoryStore = create<HistoryState>((set, get) => ({
  ...initial,

  refresh: async (repoId) => {
    if (get().repoId !== repoId) set({ ...initial, repoId });
    set({ loading: true });
    const res = await getHistory(repoId, 0, Math.max(PAGE_SIZE, get().commits.length));
    if (get().repoId !== repoId) return;
    if (res.status === "error") return set({ error: res.error, loading: false });
    const commits = res.data;
    set({ commits, complete: commits.length < PAGE_SIZE, loading: false, error: null });
    const selected = get().selectedSha;
    const next = commits.find((c) => c.sha === selected) ?? commits[0];
    if (next && next.sha !== selected) await get().selectCommit(next.sha);
    if (!next) set({ selectedSha: null, files: null, selectedFile: null });
  },

  loadMore: async () => {
    const { repoId, loading, complete, commits } = get();
    if (!repoId || loading || complete) return;
    set({ loading: true });
    const res = await getHistory(repoId, commits.length, PAGE_SIZE);
    if (get().repoId !== repoId) return;
    if (res.status === "error") return set({ error: res.error, loading: false });
    set({
      commits: [...commits, ...res.data],
      complete: res.data.length < PAGE_SIZE,
      loading: false,
    });
  },

  selectCommit: async (sha) => {
    const repoId = get().repoId;
    if (!repoId) return;
    set({ selectedSha: sha, files: null, selectedFile: null });
    const res = await getCommitFiles(repoId, sha);
    if (get().selectedSha !== sha) return;
    if (res.status === "error") return set({ error: res.error });
    set({ files: res.data, selectedFile: res.data[0] ?? null });
  },

  selectFile: (file) => set({ selectedFile: file }),
}));
