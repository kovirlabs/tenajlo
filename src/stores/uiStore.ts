import { create } from "zustand";
import type { AppError } from "../bindings";

export type Tab = "changes" | "history";
export type Dialog = "newBranch" | "accounts" | "clone" | null;

type UiState = {
  error: AppError | null;
  tab: Tab;
  dialog: Dialog;
  /** Account whose token stopped working; the Accounts dialog opens straight to re-entering it. */
  signInAgainId: string | null;
  openDialog: (dialog: Dialog) => void;
  signInAgain: (accountId: string) => void;
  showError: (error: AppError) => void;
  dismissError: () => void;
  setTab: (tab: Tab) => void;
};

export const useUiStore = create<UiState>((set) => ({
  error: null,
  tab: "changes",
  dialog: null,
  signInAgainId: null,
  openDialog: (dialog) => set({ dialog, signInAgainId: null }),
  signInAgain: (accountId) => set({ dialog: "accounts", signInAgainId: accountId, error: null }),
  showError: (error) => set({ error }),
  dismissError: () => set({ error: null }),
  setTab: (tab) => set({ tab }),
}));
