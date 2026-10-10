import { create } from "zustand";
import type { AppError } from "../bindings";

export type Tab = "changes" | "history";
export type Dialog = "newBranch" | "newRepository" | "clone" | "settings" | null;
export type SettingsTab = "accounts" | "ssh" | "git" | "appearance" | "repositories" | "about";

type UiState = {
  error: AppError | null;
  tab: Tab;
  dialog: Dialog;
  settingsTab: SettingsTab;
  /** The toolbar repository list (Ctrl/Cmd+T). */
  repoPickerOpen: boolean;
  setRepoPickerOpen: (open: boolean) => void;
  /** Account whose token stopped working; the Accounts dialog opens straight to re-entering it. */
  signInAgainId: string | null;
  openDialog: (dialog: Dialog) => void;
  openSettings: (tab?: SettingsTab) => void;
  setSettingsTab: (tab: SettingsTab) => void;
  signInAgain: (accountId: string) => void;
  showError: (error: AppError) => void;
  dismissError: () => void;
  setTab: (tab: Tab) => void;
};

export const useUiStore = create<UiState>((set) => ({
  error: null,
  tab: "changes",
  dialog: null,
  settingsTab: "accounts",
  repoPickerOpen: false,
  setRepoPickerOpen: (repoPickerOpen) => set({ repoPickerOpen }),
  signInAgainId: null,
  openDialog: (dialog) => set({ dialog, signInAgainId: null }),
  openSettings: (tab = "accounts") =>
    set({ dialog: "settings", settingsTab: tab, signInAgainId: null }),
  setSettingsTab: (settingsTab) => set({ settingsTab, signInAgainId: null }),
  signInAgain: (accountId) =>
    set({ dialog: "settings", settingsTab: "accounts", signInAgainId: accountId, error: null }),
  showError: (error) => set({ error }),
  dismissError: () => set({ error: null }),
  setTab: (tab) => set({ tab }),
}));
