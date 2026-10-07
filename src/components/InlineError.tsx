import type { AppError } from "../bindings";
import { useUiStore } from "../stores/uiStore";
import { ErrorDetails } from "./ErrorDetails";

/** An error shown inside a dialog, with "Sign in again" when an account's token stopped working. */
export function InlineError({ error }: { error: AppError | null }) {
  const signInAgain = useUiStore((s) => s.signInAgain);
  if (!error) return null;
  const accountId = error.kind === "SignInRequired" ? error.accountId : null;
  return (
    <div className="form-error" role="alert">
      {error.message}
      {accountId && (
        <>
          {" "}
          <button type="button" className="link" onClick={() => signInAgain(accountId)}>
            Sign in again
          </button>
        </>
      )}
      <ErrorDetails details={error.details} />
    </div>
  );
}
