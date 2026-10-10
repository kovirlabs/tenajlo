import { useUiStore, type Dialog } from "../stores/uiStore";

/**
 * One of the app-level dialogs (`uiStore.dialog`). `close` only closes it if it's still the
 * open one: Modal's onClose also fires when another dialog replaced this one (e.g. "Sign in"),
 * and that one must stay open.
 */
export function useDialog(dialog: NonNullable<Dialog>): { open: boolean; close: () => void } {
  const open = useUiStore((s) => s.dialog === dialog);
  const close = () => {
    const ui = useUiStore.getState();
    if (ui.dialog === dialog) ui.openDialog(null);
  };
  return { open, close };
}
