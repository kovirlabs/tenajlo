import { useEffect, useState } from "react";
import type { Account } from "../../bindings";
import { useAccountStore } from "../../stores/accountStore";
import { useUiStore } from "../../stores/uiStore";
import { Modal } from "../Modal";
import { SignInForm } from "./SignInForm";

/** Lists Forgejo accounts, with sign in and sign out. Settings (M6) will absorb this. */
export function AccountsDialog() {
  const open = useUiStore((s) => s.dialog === "accounts");
  const close = () => useUiStore.getState().openDialog(null);
  return (
    <Modal open={open} title="Forgejo accounts" onClose={close}>
      <AccountsBody onClose={close} />
    </Modal>
  );
}

/** Mounted only while the dialog is open, so its state resets on close. */
function AccountsBody({ onClose }: { onClose: () => void }) {
  const accounts = useAccountStore((s) => s.accounts);
  const loaded = useAccountStore((s) => s.loaded);
  const load = useAccountStore((s) => s.load);
  const signOut = useAccountStore((s) => s.signOut);
  const [signingIn, setSigningIn] = useState(false);
  const [confirming, setConfirming] = useState<string | null>(null);

  useEffect(() => {
    void load();
  }, [load]);

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
  onCancel: () => void;
  onConfirm: () => void;
};

function AccountRow({ account, confirming, onSignOut, onCancel, onConfirm }: RowProps) {
  return (
    <li className="account-row">
      <div>
        <strong>{account.displayName}</strong>
        {account.displayName !== account.login && <span className="muted"> ({account.login})</span>}
        <div className="muted">{account.baseUrl}</div>
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
        <button type="button" className="secondary" onClick={onSignOut}>
          Sign out
        </button>
      )}
    </li>
  );
}
