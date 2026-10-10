// Renders the real Tenajlo UI with Tauri's IPC mocked and invented data, for screenshots.
// Open /demo/?scene=changes|history|clone|conflict|welcome|ssh|passwords|update[&theme=dark]
// on the Vite dev server. Not part of the app build.
import { createRoot } from "react-dom/client";
import { mockIPC } from "@tauri-apps/api/mocks";
import { App } from "../src/App";
import { useHistoryStore } from "../src/stores/historyStore";
import { useUiStore } from "../src/stores/uiStore";
import * as f from "./fixtures";
import "../src/styles.css";

const params = new URLSearchParams(location.search);
const scene = params.get("scene") ?? "changes";
const dark = params.get("theme") === "dark";

const conflict = scene === "conflict";
const welcome = scene === "welcome";

mockIPC(
  (cmd, args) => {
    const path = (args as { path?: string } | undefined)?.path;
    switch (cmd) {
      case "check_git":
        return {
          path: "C:\\Program Files\\Tenajlo\\mingit\\cmd\\git.exe",
          version: { major: 2, minor: 56, patch: 0 },
          minimum: { major: 2, minor: 40, patch: 0 },
          supported: true,
          lfsVersion: "git-lfs/3.8.0",
        };
      case "get_settings":
        return {
          ...f.settings,
          theme: dark ? "Dark" : "Light",
          welcomeCompleted: !welcome,
          checkForUpdates: scene === "update",
        };
      case "list_repositories":
        return welcome
          ? { repositories: [], selectedId: null }
          : {
              repositories: [f.repository, ...f.otherRepositories],
              selectedId: f.repository.id,
            };
      case "list_accounts":
        return welcome ? [] : [f.account];
      case "get_status":
        return conflict ? f.conflictStatus : f.status;
      case "get_working_diff":
        return {
          diff: path?.endsWith(".SLDPRT") ? { type: "Binary" } : f.pumpControlDiff,
          lines: null,
        };
      case "get_history":
        return f.history;
      case "get_commit_files":
        return f.commitFiles;
      case "get_commit_diff":
        return f.pumpControlDiff;
      case "get_branches":
        return f.branches;
      case "get_sync_state":
        return f.syncState;
      case "get_operation_state":
        return conflict ? f.conflictOperation : f.noOperation;
      case "get_lfs_status":
        return { used: true, installed: true };
      case "get_saved_changes":
        return null;
      case "get_identity":
      case "get_global_identity":
        return { name: "Sam Rivera", email: "sam@example.com" };
      case "list_forgejo_repositories":
        return f.remoteRepositories;
      case "suggest_clone_path":
        return "C:\\Users\\sam\\Documents\\Tenajlo\\packaging-cell";
      case "check_server":
        return null;
      case "list_ssh_keys":
        return f.sshKeys;
      case "get_account_ssh_keys":
        return {
          accountId: f.account.id,
          canAdd: true,
          fingerprints: [f.sshKeys[0]?.fingerprint],
          requiredScopes: ["write:user", "read:repository", "write:repository"],
        };
      case "list_saved_secrets":
        return f.savedSecrets;
      case "get_app_info":
        return {
          version: "1.0.2",
          logsDir: "C:\\Users\\sam\\AppData\\Roaming\\com.kovirlabs.tenajlo\\logs",
          canUpdate: true,
        };
      case "check_for_update":
        return scene === "update"
          ? {
              version: "1.1.0",
              notes: null,
              releaseUrl: "https://github.com/kovirlabs/tenajlo/releases/latest",
            }
          : null;
      default:
        return null;
    }
  },
  { shouldMockEvents: true },
);

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");
createRoot(root).render(<App />);

/** Runs `fn` once `ready()` holds (data from the mocked IPC has arrived). */
function when(ready: () => boolean, fn: () => void) {
  const timer = setInterval(() => {
    if (!ready()) return;
    clearInterval(timer);
    fn();
  }, 20);
}

if (scene === "history") {
  useUiStore.setState({ tab: "history" });
  when(
    () => useHistoryStore.getState().commits.length > 0,
    () => {
      const first = f.history[0];
      if (first) void useHistoryStore.getState().selectCommit(first.sha);
    },
  );
}

if (scene === "clone") {
  when(
    () => document.querySelector(".shell") !== null,
    () => useUiStore.getState().openDialog("clone"),
  );
  // Pick a repository so the dialog shows its destination folder.
  when(
    () => document.querySelectorAll(".clone-repo-list .picker-item").length > 2,
    () => document.querySelectorAll<HTMLButtonElement>(".clone-repo-list .picker-item")[2]?.click(),
  );
}

if (scene === "ssh") useUiStore.getState().openSettings("ssh");
if (scene === "passwords") useUiStore.getState().openSettings("passwords");
if (scene === "update") {
  useUiStore.getState().openSettings("about");
}

if (scene === "changes") {
  // Type a commit summary the way a user would, so the commit button is ready.
  when(
    () => document.querySelector(".commit-summary-input") !== null,
    () => {
      const input = document.querySelector<HTMLInputElement>(".commit-summary-input");
      const setValue = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
      if (!input || !setValue) return;
      setValue.call(input, "Add low suction pressure alarm");
      input.dispatchEvent(new Event("input", { bubbles: true }));
    },
  );
}
