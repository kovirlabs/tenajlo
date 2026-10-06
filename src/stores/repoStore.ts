import { create } from "zustand";
import type { Repository } from "../bindings";
import * as api from "../api/repos";
import { useUiStore } from "./uiStore";

type RepoState = {
  repositories: Repository[];
  selectedId: string | null;
  loaded: boolean;
  load: () => Promise<void>;
  addLocal: () => Promise<void>;
  remove: (id: string) => Promise<void>;
  select: (id: string) => Promise<void>;
};

const showError = useUiStore.getState().showError;

export const useRepoStore = create<RepoState>((set, get) => ({
  repositories: [],
  selectedId: null,
  loaded: false,

  load: async () => {
    const list = await api.listRepositories();
    set({ repositories: list.repositories, selectedId: list.selectedId, loaded: true });
  },

  addLocal: async () => {
    const res = await api.addLocalRepository();
    if (res.status === "error") return showError(res.error);
    if (res.data) await get().load();
  },

  remove: async (id) => {
    const res = await api.removeRepository(id);
    if (res.status === "error") return showError(res.error);
    await get().load();
  },

  select: async (id) => {
    const res = await api.selectRepository(id);
    if (res.status === "error") return showError(res.error);
    await get().load();
  },
}));

/** The selected repository, if any. */
export function useSelectedRepository(): Repository | null {
  return useRepoStore((s) => s.repositories.find((r) => r.id === s.selectedId) ?? null);
}
