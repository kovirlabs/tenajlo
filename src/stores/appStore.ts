import { create } from "zustand";
import type { AppError, GitInfo } from "../bindings";
import { checkGit } from "../api/git";

export type GitCheck =
  | { phase: "checking" }
  | { phase: "ready"; info: GitInfo }
  | { phase: "unsupported"; info: GitInfo }
  | { phase: "failed"; error: AppError };

type AppState = {
  gitCheck: GitCheck;
  runGitCheck: () => Promise<void>;
  /** Re-checks after Settings changes the git program, without blanking the app first. */
  recheckGit: () => Promise<void>;
};

export const useAppStore = create<AppState>((set, get) => ({
  gitCheck: { phase: "checking" },
  runGitCheck: async () => {
    set({ gitCheck: { phase: "checking" } });
    await get().recheckGit();
  },
  recheckGit: async () => {
    const res = await checkGit();
    if (res.status === "error") {
      set({ gitCheck: { phase: "failed", error: res.error } });
    } else if (res.data.supported) {
      set({ gitCheck: { phase: "ready", info: res.data } });
    } else {
      set({ gitCheck: { phase: "unsupported", info: res.data } });
    }
  },
}));
