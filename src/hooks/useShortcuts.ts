import { useEffect } from "react";
import { useSyncStore } from "../stores/syncStore";
import { useUiStore } from "../stores/uiStore";

/** A dialog is showing: one the UI store tracks (Settings, Clone…) or any other modal, such
 *  as a confirmation, an error or a sign-in prompt, which the store doesn't know about. */
function dialogOpen(): boolean {
  return useUiStore.getState().dialog !== null || document.querySelector("dialog[open]") !== null;
}

/**
 * App keyboard shortcuts (spec §8.2). Ctrl/Cmd+Shift+N: new branch; Ctrl/Cmd+Shift+P: push;
 * Ctrl/Cmd+T: repository list; Ctrl/Cmd+,: settings.
 */
export function useShortcuts(repoOpen: boolean) {
  const openDialog = useUiStore((s) => s.openDialog);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.shiftKey || e.altKey) return;
      const ui = useUiStore.getState();
      if (dialogOpen()) return;
      if (e.key === ",") {
        e.preventDefault();
        ui.openSettings();
      } else if (e.key.toLowerCase() === "t") {
        e.preventDefault();
        ui.setRepoPickerOpen(!ui.repoPickerOpen);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  useEffect(() => {
    if (!repoOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || !e.shiftKey) return;
      // Never act behind an open dialog (a push from inside Settings or under an error, say).
      if (dialogOpen()) return;
      const key = e.key.toLowerCase();
      if (key === "n") {
        e.preventDefault();
        openDialog("newBranch");
      } else if (key === "p") {
        e.preventDefault();
        const sync = useSyncStore.getState();
        const action = sync.state?.action.type;
        if (action === "Push" || action === "Publish") void sync.run(action);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [repoOpen, openDialog]);
}
