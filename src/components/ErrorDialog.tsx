import { useEffect, useRef } from "react";
import { useUiStore } from "../stores/uiStore";
import { ErrorDetails } from "./ErrorDetails";

/** Modal for command errors: plain message first, raw output behind "Details" (spec §8.2). */
export function ErrorDialog() {
  const error = useUiStore((s) => s.error);
  const dismiss = useUiStore((s) => s.dismissError);
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    // jsdom lacks showModal; guard so tests can render the dialog.
    if (error && !dialog.open) dialog.showModal?.();
    if (!error && dialog.open) dialog.close?.();
  }, [error]);

  return (
    <dialog ref={ref} className="dialog" onClose={dismiss} aria-labelledby="error-title">
      {error && (
        <>
          <h2 id="error-title">Something went wrong</h2>
          <p>{error.message}</p>
          <ErrorDetails details={error.details} />
          <div className="dialog-actions">
            <button type="button" onClick={dismiss} autoFocus>
              OK
            </button>
          </div>
        </>
      )}
    </dialog>
  );
}
