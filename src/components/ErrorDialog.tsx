import { useUiStore } from "../stores/uiStore";
import { ErrorDetails } from "./ErrorDetails";
import { Modal } from "./Modal";

/** Modal for command errors: plain message first, raw output behind "Details" (spec §8.2). */
export function ErrorDialog() {
  const error = useUiStore((s) => s.error);
  const dismiss = useUiStore((s) => s.dismissError);
  return (
    <Modal open={error !== null} title="Something went wrong" onClose={dismiss}>
      {error && (
        <>
          <p>{error.message}</p>
          <ErrorDetails details={error.details} />
          <div className="dialog-actions">
            <button type="button" onClick={dismiss} autoFocus>
              OK
            </button>
          </div>
        </>
      )}
    </Modal>
  );
}
