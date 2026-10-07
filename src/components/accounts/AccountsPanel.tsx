import { useEffect, useState } from "react";
import type { Account } from "../../bindings";
import { useAccountStore } from "../../stores/accountStore";
import { useUiStore } from "../../stores/uiStore";
import { SignInForm } from "./SignInForm";

/** Settings → Accounts: list, sign in, sign out, sign in again. Mounted only while shown. */
export function AccountsPanel({ onClose }: { onClose: () => void }) {
  const accounts = useAccountStore((s) => s.accounts);
  const loaded = useAccountStore((s) => s.loaded);
  const load = useAccountStore((s) => s.load);
  const signOut = useAccountStore((s) => s.signOut);
  const [signingIn, setSigningIn] = useState(false);
  const [confirming, setConfirming] = useState<string | null>(null);
  const initialReauth = useUiStore((s) => s.signInAgainId);
  const [reauthId, setReauthId] = useState(initialReauth);
  const reauth = accounts.find((a) => a.id === reauthId) ?? null;

  useEffect(() => {
    void load();
  }, [load]);

  if (reauth) {
    return (
      <>
        <p>
          Your sign-in for <strong>{reauth.baseUrl}</strong> ({reauth.login}) has stopped working.
          Create a new access token and paste it below.
        </p>
        <SignInForm
          initialServer={reauth.baseUrl}
          onSignedIn={() => {
            void load().then(() => setReauthId(null));
          }}
          onCancel={() => setReauthId(null)}
        />
      </>
    );
  }
  // Waiting for the list before deciding between "sign in again" and the list.
  if (reauthId && !loaded) return null;

  if (signingIn || (loaded && accounts.length === 0)) {
    return (
      <>
        {accounts.length === 0 && (
          <p>
            Sign in to your Forgejo server to clone its repositories and push without passwords.
          </p>
        )}
        <SignInForm
          onSignedIn={() => {
            void load().then(() => setSigningIn(false));
          }}
          onCancel={accounts.length === 0 ? onClose : () => setSigningIn(false)}
        />
      </>
    );
  }

  return (
    <div className="form">
      <ul className="account-list">
        {accounts.map((a) => (
          <AccountRow
            key={a.id}
            account={a}
            confirming={confirming === a.id}
            onSignOut={() => setConfirming(a.id)}
            onSignInAgain={() => setReauthId(a.id)}
            onCancel={() => setConfirming(null)}
            onConfirm={() => {
              setConfirming(null);
              void signOut(a.id);
            }}
          />
        ))}
      </ul>
      <div className="dialog-actions">
        <button type="button" className="secondary" onClick={() => setSigningIn(true)}>
          Sign in to another server…
        </button>
        <button type="button" onClick={onClose}>
          Done
        </button>
      </div>
    </div>
  );
}

type RowProps = {
  account: Account;
  confirming: boolean;
  onSignOut: () => void;
  onSignInAgain: () => void;
  onCancel: () => void;
  onConfirm: () => void;
};

function AccountRow({
  account,
  confirming,
  onSignOut,
  onSignInAgain,
  onCancel,
  onConfirm,
}: RowProps) {
  return (
    <li className="account-row">
      <div>
        <strong>{account.displayName}</strong>
        {account.displayName !== account.login && <span className="muted"> ({account.login})</span>}
        <div className="muted">{account.baseUrl}</div>
        {account.needsSignIn && <div className="form-error">Sign-in has stopped working</div>}
      </div>
      {confirming ? (
        <div className="account-confirm">
          <span>Sign out? The saved token will be removed from this computer.</span>
          <button type="button" className="secondary" onClick={onCancel}>
            Cancel
          </button>
          <button type="button" className="danger" onClick={onConfirm}>
            Sign out
          </button>
        </div>
      ) : (
        <div className="account-actions">
          {account.needsSignIn && (
            <button type="button" onClick={onSignInAgain}>
              Sign in again
            </button>
          )}
          <button type="button" className="secondary" onClick={onSignOut}>
            Sign out
          </button>
        </div>
      )}
    </li>
  );
}
