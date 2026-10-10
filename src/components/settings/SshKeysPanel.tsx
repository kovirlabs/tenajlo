import { useState } from "react";
import type { Account, AccountSshKeys, AppError, LocalSshKey } from "../../bindings";
import { addSshKey, createSshKey, getAccountSshKeys, listSshKeys } from "../../api/sshKeys";
import { useAsyncEffect } from "../../hooks/useAsyncEffect";
import { useAccountStore } from "../../stores/accountStore";
import { SignInForm } from "../accounts/SignInForm";
import { InlineError } from "../InlineError";

/** What each signed-in account allows, by account id. */
type AccessMap = Record<string, AccountSshKeys | AppError>;

/**
 * Settings → SSH keys (spec §6.3): the keys in `.ssh`, creating `id_ed25519`, and adding a key
 * to a Forgejo account. "Add" only appears once Rust has confirmed the account's token may
 * add keys; otherwise the panel explains how to get a token that can.
 */
export function SshKeysPanel() {
  const accounts = useAccountStore((s) => s.accounts);
  const loadAccounts = useAccountStore((s) => s.load);
  const [keys, setKeys] = useState<LocalSshKey[] | null>(null);
  const [access, setAccess] = useState<AccessMap>({});
  const [error, setError] = useState<AppError | null>(null);
  const [creating, setCreating] = useState(false);
  const [updating, setUpdating] = useState<Account | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [reload, setReload] = useState(0);

  useAsyncEffect(async () => {
    await loadAccounts();
  }, [loadAccounts]);

  useAsyncEffect(
    async (live) => {
      const listed = await listSshKeys();
      if (!live()) return;
      if (listed.status === "error") setError(listed.error);
      else setKeys(listed.data);
    },
    [reload],
  );

  useAsyncEffect(
    async (live) => {
      const usable = accounts.filter((a) => !a.needsSignIn);
      const results = await Promise.all(usable.map((a) => getAccountSshKeys(a.id)));
      if (!live()) return;
      const next: AccessMap = {};
      usable.forEach((a, i) => {
        const res = results[i];
        if (res) next[a.id] = res.status === "ok" ? res.data : res.error;
      });
      setAccess(next);
    },
    [accounts, reload],
  );

  const refresh = () => setReload((n) => n + 1);

  const add = async (account: Account, key: LocalSshKey) => {
    setBusy(`${account.id}/${key.fileName}`);
    setError(null);
    const title = key.comment.trim() || key.fileName.replace(/\.pub$/, "");
    const res = await addSshKey(account.id, key.fileName, title);
    setBusy(null);
    if (res.status === "error") setError(res.error);
    refresh();
  };

  if (updating) {
    const scopes = Object.values(access).find((a) => "requiredScopes" in a)?.requiredScopes;
    return (
      <>
        <p>
          Create a new access token for <strong>{updating.baseUrl}</strong> that can add SSH keys,
          then paste it below. It replaces the token Tenajlo saved before.
        </p>
        <SignInForm
          initialServer={updating.baseUrl}
          {...(scopes && { scopes })}
          onSignedIn={() => {
            setUpdating(null);
            void loadAccounts();
          }}
          onCancel={() => setUpdating(null)}
        />
      </>
    );
  }

  if (creating) {
    return (
      <CreateKeyForm
        onCreated={() => {
          setCreating(false);
          refresh();
        }}
        onCancel={() => setCreating(false)}
      />
    );
  }

  const hasDefaultKey = keys?.some((k) => k.fileName === "id_ed25519.pub") ?? false;
  const forgejo = accounts.filter((a) => !a.needsSignIn);

  return (
    <div className="form">
      <p>
        An SSH key lets you clone, pull and push with SSH addresses instead of a password. Keys live
        in the <code>.ssh</code> folder in your home folder.
      </p>
      {keys === null ? (
        !error && <p className="muted">Looking for keys…</p>
      ) : keys.length === 0 ? (
        <p className="muted">There are no SSH keys on this computer yet.</p>
      ) : (
        <ul className="account-list">
          {keys.map((k) => (
            <li key={k.fileName} className="account-row">
              <div>
                <strong>{k.fileName}</strong>
                {k.comment && <span className="muted"> ({k.comment})</span>}
                <div className="muted ssh-fingerprint">{k.fingerprint}</div>
              </div>
              <div className="ssh-key-accounts">
                {forgejo.map((a) => (
                  <KeyStatus
                    key={a.id}
                    account={a}
                    sshKey={k}
                    access={access[a.id]}
                    busy={busy === `${a.id}/${k.fileName}`}
                    onAdd={() => void add(a, k)}
                  />
                ))}
              </div>
            </li>
          ))}
        </ul>
      )}
      {forgejo.map((a) => {
        const status = access[a.id];
        if (!status) return null;
        if ("message" in status) return <InlineError key={a.id} error={status} />;
        if (status.canAdd) return null;
        return (
          <p key={a.id} className="muted">
            The token Tenajlo saved for {a.baseUrl} doesn&apos;t have permission to add SSH keys.{" "}
            <button type="button" className="link" onClick={() => setUpdating(a)}>
              Update the token
            </button>{" "}
            to add keys from here, or add them on the website.
          </p>
        );
      })}
      <InlineError error={error} />
      {!hasDefaultKey && keys !== null && (
        <div className="dialog-actions">
          <button type="button" onClick={() => setCreating(true)}>
            Create an SSH key…
          </button>
        </div>
      )}
    </div>
  );
}

type StatusProps = {
  account: Account;
  sshKey: LocalSshKey;
  access: AccountSshKeys | AppError | undefined;
  busy: boolean;
  onAdd: () => void;
};

/** Whether one key is on one account, with "Add" when the account's token allows it. */
function KeyStatus({ account, sshKey, access, busy, onAdd }: StatusProps) {
  const host = new URL(account.baseUrl).host;
  if (!access || "message" in access) return null;
  if (access.fingerprints.includes(sshKey.fingerprint)) {
    return <span className="muted">Added to {host}</span>;
  }
  if (!access.canAdd) return null;
  return (
    <button type="button" className="secondary" disabled={busy} onClick={onAdd}>
      {busy ? "Adding…" : `Add to ${host}`}
    </button>
  );
}

/** Creates `.ssh/id_ed25519`, optionally protected by a passphrase. */
function CreateKeyForm({ onCreated, onCancel }: { onCreated: () => void; onCancel: () => void }) {
  const [name, setName] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const mismatch = passphrase !== confirm;

  const submit = async () => {
    setBusy(true);
    setError(null);
    const res = await createSshKey(name, passphrase || null);
    setBusy(false);
    if (res.status === "error") setError(res.error);
    else onCreated();
  };

  return (
    <form
      className="form"
      onSubmit={(e) => {
        e.preventDefault();
        if (!mismatch) void submit();
      }}
    >
      <label>
        Name
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g. Work laptop"
          autoFocus
        />
      </label>
      <p className="muted">The name helps you recognize this key on the server later.</p>
      <label>
        Passphrase (optional)
        <input
          type="password"
          value={passphrase}
          onChange={(e) => setPassphrase(e.target.value)}
          autoComplete="new-password"
        />
      </label>
      <label>
        Confirm passphrase
        <input
          type="password"
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          autoComplete="new-password"
        />
      </label>
      <p className={mismatch && confirm ? "form-error" : "muted"}>
        {mismatch && confirm
          ? "The passphrases don't match."
          : "A passphrase protects the key if someone copies it. You'll be asked for it when you pull or push."}
      </p>
      <InlineError error={error} />
      <div className="dialog-actions">
        <button type="button" className="secondary" onClick={onCancel}>
          Cancel
        </button>
        <button type="submit" disabled={busy || mismatch}>
          {busy ? "Creating…" : "Create key"}
        </button>
      </div>
    </form>
  );
}
