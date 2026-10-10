import { create } from "zustand";
import type { AppError, AvailableUpdate } from "../bindings";
import * as api from "../api/updates";

type UpdateState = {
  available: AvailableUpdate | null;
  /** The last manual check found nothing newer. */
  upToDate: boolean;
  checking: boolean;
  installing: boolean;
  /** Download progress while installing; null when the size is unknown. */
  percent: number | null;
  error: AppError | null;
  /** "Later" hides the banner until the next start. */
  dismissed: boolean;
  /** `quiet` (the startup check) doesn't report errors. */
  check: (quiet: boolean) => Promise<void>;
  install: () => Promise<void>;
  dismiss: () => void;
};

export const useUpdateStore = create<UpdateState>((set, get) => ({
  available: null,
  upToDate: false,
  checking: false,
  installing: false,
  percent: null,
  error: null,
  dismissed: false,

  check: async (quiet) => {
    if (get().checking) return;
    set({ checking: true, error: null, upToDate: false });
    const res = await api.checkForUpdate();
    if (res.status === "ok") {
      set({ checking: false, available: res.data, upToDate: res.data === null });
    } else {
      set({ checking: false, error: quiet ? null : res.error });
    }
  },

  install: async () => {
    set({ installing: true, percent: null, error: null });
    const stop = await api.onUpdateProgress((p) => set({ percent: p.percent }));
    // On success Tenajlo restarts and this never returns.
    const res = await api.installUpdate();
    stop();
    if (res.status === "error") set({ installing: false, error: res.error });
  },

  dismiss: () => set({ dismissed: true }),
}));
