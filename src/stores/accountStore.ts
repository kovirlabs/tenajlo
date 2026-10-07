import { create } from "zustand";
import type { Account } from "../bindings";
import * as api from "../api/accounts";
import { useUiStore } from "./uiStore";

type AccountState = {
  accounts: Account[];
  loaded: boolean;
  load: () => Promise<void>;
  signOut: (id: string) => Promise<void>;
};

export const useAccountStore = create<AccountState>((set, get) => ({
  accounts: [],
  loaded: false,

  load: async () => {
    set({ accounts: await api.listAccounts(), loaded: true });
  },

  signOut: async (id) => {
    const res = await api.signOut(id);
    if (res.status === "error") useUiStore.getState().showError(res.error);
    await get().load();
  },
}));
