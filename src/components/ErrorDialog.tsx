import { useUiStore } from "../stores/uiStore";
import { ErrorDetails } from "./ErrorDetails";
import { Modal } from "./Modal";

/** Modal for command errors: plain message first, raw output behind "Details" (spec §8.2). */
export function ErrorDialog() {
  const error = useUiStore((s) => s.error);
  const dismiss = useUiStore((s) => s.dismissError);
  const signInAgain = useUiStore((s) => s.signInAgain);
  const reauthId = error?.kind === "SignInRequired" ? error.accountId : null;
  return (
    <Modal open={error !== null} title="Something went wrong" onClose={dismiss}>
      {error && (
        <>
          <p>{error.message}</p>
          <ErrorDetails details={error.details} />
          <div className="dialog-actions">
            {reauthId ? (
              <>
                <button type="button" className="secondary" onClick={dismiss}>
                  Not now
                </button>
                <button type="button" onClick={() => signInAgain(reauthId)} autoFocus>
                  Sign in again
                </button>
              </>
            ) : (
              <button type="button" onClick={dismiss} autoFocus>
                OK
              </button>
            )}
          </div>
        </>
      )}
    </Modal>
  );
}
