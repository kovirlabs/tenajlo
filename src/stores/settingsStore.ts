import { create } from "zustand";
import type { AppError, Settings, Theme } from "../bindings";
import * as api from "../api/settings";

type SettingsState = {
  settings: Settings | null;
  load: () => Promise<void>;
  /** Saves a change; returns the error to show next to the control, if any. */
  update: (change: Partial<Settings>) => Promise<AppError | null>;
};

/** `data-theme` on <html> overrides the OS light/dark preference (styles.css). */
export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  if (theme === "System") delete root.dataset.theme;
  else root.dataset.theme = theme.toLowerCase();
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: null,

  load: async () => {
    const settings = await api.getSettings();
    applyTheme(settings.theme);
    set({ settings });
  },

  update: async (change) => {
    const current = get().settings;
    if (!current) return null;
    const res = await api.saveSettings({ ...current, ...change });
    if (res.status === "error") return res.error;
    applyTheme(res.data.theme);
    set({ settings: res.data });
    return null;
  },
}));
