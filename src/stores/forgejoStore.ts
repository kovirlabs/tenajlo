import { create } from "zustand";
import type { BranchLink, ForgejoPage } from "../bindings";
import { getBranchLink, openForgejoPage } from "../api/forgejo";
import { useUiStore } from "./uiStore";

type ForgejoState = {
  repoId: string | null;
  branch: string | null;
  /** The current branch on Forgejo; `null` when the repository isn't linked (or unknown yet). */
  link: BranchLink | null;
  /** Reloads the link. Failures hide the Forgejo links rather than showing an error. */
  load: (repoId: string, branch: string | null) => Promise<void>;
  /** Opens a page; errors (e.g. sign in again) go to the error dialog. */
  open: (page: ForgejoPage) => Promise<void>;
};

export const useForgejoStore = create<ForgejoState>((set, get) => ({
  repoId: null,
  branch: null,
  link: null,

  load: async (repoId, branch) => {
    if (get().repoId !== repoId || get().branch !== branch) set({ repoId, branch, link: null });
    if (branch === null) return;
    const res = await getBranchLink(repoId, branch);
    if (get().repoId !== repoId || get().branch !== branch) return;
    set({ link: res.status === "ok" ? res.data : null });
  },

  open: async (page) => {
    const repoId = get().repoId;
    if (!repoId) return;
    const res = await openForgejoPage(repoId, page);
    if (res.status === "error") useUiStore.getState().showError(res.error);
  },
}));
