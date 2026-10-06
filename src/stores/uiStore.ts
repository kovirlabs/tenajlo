import { create } from "zustand";
import type { AppError } from "../bindings";

export type Tab = "changes" | "history";

type UiState = {
  error: AppError | null;
  tab: Tab;
  showError: (error: AppError) => void;
  dismissError: () => void;
  setTab: (tab: Tab) => void;
};

export const useUiStore = create<UiState>((set) => ({
  error: null,
  tab: "changes",
  showError: (error) => set({ error }),
  dismissError: () => set({ error: null }),
  setTab: (tab) => set({ tab }),
}));
