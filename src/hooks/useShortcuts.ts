import { useEffect } from "react";
import { useUiStore } from "../stores/uiStore";

/** App keyboard shortcuts (spec §8.2). Ctrl/Cmd+Shift+N: new branch. */
export function useShortcuts(repoOpen: boolean) {
  const openDialog = useUiStore((s) => s.openDialog);
  useEffect(() => {
    if (!repoOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "n") {
        e.preventDefault();
        openDialog("newBranch");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [repoOpen, openDialog]);
}
