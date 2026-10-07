import { create } from "zustand";
import type { AppError } from "../bindings";

export type Tab = "changes" | "history";
export type Dialog = "newBranch" | "accounts" | null;

type UiState = {
  error: AppError | null;
  tab: Tab;
  dialog: Dialog;
  openDialog: (dialog: Dialog) => void;
  showError: (error: AppError) => void;
  dismissError: () => void;
  setTab: (tab: Tab) => void;
};

export const useUiStore = create<UiState>((set) => ({
  error: null,
  tab: "changes",
  dialog: null,
  openDialog: (dialog) => set({ dialog }),
  showError: (error) => set({ error }),
  dismissError: () => set({ error: null }),
  setTab: (tab) => set({ tab }),
}));
