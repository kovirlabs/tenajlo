import { useEffect } from "react";
import { useSyncStore } from "../stores/syncStore";
import { useUiStore } from "../stores/uiStore";

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
      if (e.key === ",") {
        e.preventDefault();
        ui.openSettings();
      } else if (e.key.toLowerCase() === "t" && ui.dialog === null) {
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
