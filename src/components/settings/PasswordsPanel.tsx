import { useState } from "react";
import type { AppError, SavedSecretInfo } from "../../bindings";
import { forgetSavedSecret, listSavedSecrets } from "../../api/savedSecrets";
import { useAsyncEffect } from "../../hooks/useAsyncEffect";
import { InlineError } from "../InlineError";

/**
 * Settings → Passwords: what the user ticked "Remember" for in a sign-in or SSH passphrase
 * dialog. Forgetting deletes it from the keychain; Tenajlo asks again next time.
 */
export function PasswordsPanel() {
  const [saved, setSaved] = useState<SavedSecretInfo[] | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [reload, setReload] = useState(0);

  useAsyncEffect(
    async (live) => {
      const list = await listSavedSecrets();
      if (live()) setSaved(list);
    },
    [reload],
  );

  const forget = async (id: string) => {
    setError(null);
    const res = await forgetSavedSecret(id);
    if (res.status === "error") setError(res.error);
    setReload((n) => n + 1);
  };

  return (
    <div className="form">
      <p>
        Passwords and SSH key passphrases you chose to remember. They&apos;re kept in this
        computer&apos;s password store. Access tokens for your Forgejo accounts are under Accounts.
      </p>
      {saved?.length === 0 && <p className="muted">Nothing is remembered yet.</p>}
      {saved && saved.length > 0 && (
        <ul className="account-list">
          {saved.map((s) => (
            <li key={s.id} className="account-row">
              <div>
                {s.type === "Login" ? (
                  <>
                    <strong>{s.username}</strong>
                    <div className="muted">Password for {s.host}</div>
                  </>
                ) : (
                  <>
                    <strong>{baseName(s.key)}</strong>
                    <div className="muted">Passphrase for the SSH key {s.key}</div>
                  </>
                )}
              </div>
              <button type="button" className="secondary" onClick={() => void forget(s.id)}>
                Forget
              </button>
            </li>
          ))}
        </ul>
      )}
      <InlineError error={error} />
    </div>
  );
}

/** Last part of a path, with either kind of slash. */
function baseName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}
