import { useState } from "react";
import type { Account, AppError, PublishOptions, Repository } from "../bindings";
import { getPublishOptions, publishRepository } from "../api/publish";
import { useAsyncEffect } from "../hooks/useAsyncEffect";
import { useDialog } from "../hooks/useDialog";
import { useAccountStore } from "../stores/accountStore";
import { refreshRepo } from "../stores/refreshRepo";
import { useSelectedRepository } from "../stores/repoStore";
import { useSyncStore } from "../stores/syncStore";
import { useUiStore } from "../stores/uiStore";
import { SignInForm } from "./accounts/SignInForm";
import { InlineError } from "./InlineError";
import { Modal } from "./Modal";

/**
 * "Publish repository" (spec §8.1, v1.2): create the repository on Forgejo for a local one
 * with no remote, then push the current branch with the normal sync.
 */
export function PublishRepositoryDialog() {
  const { open, close } = useDialog("publishRepository");
  const repo = useSelectedRepository();
  return (
    <Modal open={open} title="Publish repository" onClose={close}>
      {repo && <PublishBody repo={repo} onClose={close} />}
    </Modal>
  );
}

/** Mounted only while the dialog is open, so its state resets on close. */
function PublishBody({ repo, onClose }: { repo: Repository; onClose: () => void }) {
  const allAccounts = useAccountStore((s) => s.accounts);
  const loadAccounts = useAccountStore((s) => s.load);
  const accounts = allAccounts.filter((a) => !a.needsSignIn);
  const [accountId, setAccountId] = useState<string | null>(null);
  const account = accounts.find((a) => a.id === accountId) ?? accounts[0];
  const [options, setOptions] = useState<PublishOptions | AppError | null>(null);
  const [updating, setUpdating] = useState<Account | null>(null);
  const [reload, setReload] = useState(0);

  useAsyncEffect(async () => {
    await loadAccounts();
  }, [loadAccounts]);

  useAsyncEffect(
    async (live) => {
      setOptions(null);
      if (!account) return;
      const res = await getPublishOptions(account.id);
      if (live()) setOptions(res.status === "ok" ? res.data : res.error);
    },
    [account?.id, reload],
  );

  if (updating) {
    const scopes = options && "requiredScopes" in options ? options.requiredScopes : undefined;
    return (
      <>
        <p>
          Create a new access token for <strong>{updating.baseUrl}</strong> that can create
          repositories, then paste it below. It replaces the token Tenajlo saved before.
        </p>
        <SignInForm
          initialServer={updating.baseUrl}
          {...(scopes && { scopes })}
          onSignedIn={() => {
            setUpdating(null);
            void loadAccounts();
            setReload((n) => n + 1);
          }}
          onCancel={() => setUpdating(null)}
        />
      </>
    );
  }

  if (!account) {
    return (
      <div className="form">
        <p>Sign in to your Forgejo server to publish this repository there.</p>
        <div className="dialog-actions">
          <button type="button" className="secondary" onClick={onClose}>
            Cancel
          </button>
          <button type="button" onClick={() => useUiStore.getState().openSettings("accounts")}>
            Sign in…
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="form">
      {accounts.length > 1 && (
        <label>
          Server
          <select value={account.id} onChange={(e) => setAccountId(e.target.value)}>
            {accounts.map((a) => (
              <option key={a.id} value={a.id}>
                {a.baseUrl}
              </option>
            ))}
          </select>
        </label>
      )}
      {options === null ? (
        <p className="muted">Checking what you can publish to…</p>
      ) : "message" in options ? (
        <InlineError error={options} />
      ) : (
        <PublishForm
          key={options.accountId}
          repo={repo}
          options={options}
          onUpdateToken={() => setUpdating(account)}
          onClose={onClose}
        />
      )}
    </div>
  );
}

type FormProps = {
  repo: Repository;
  options: PublishOptions;
  onUpdateToken: () => void;
  onClose: () => void;
};

/** Owner, name and visibility, once Rust has said where the token may create repositories. */
function PublishForm({ repo, options, onUpdateToken, onClose }: FormProps) {
  const owners = [...(options.userAllowed ? [""] : []), ...options.organizations];
  const [owner, setOwner] = useState(owners[0] ?? "");
  const [name, setName] = useState(repo.name);
  const [description, setDescription] = useState("");
  const [isPrivate, setIsPrivate] = useState(true);
  const [ssh, setSsh] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  if (owners.length === 0) {
    return (
      <>
        <p>
          The token Tenajlo saved for this server doesn&apos;t have permission to create
          repositories.{" "}
          <button type="button" className="link" onClick={onUpdateToken}>
            Update the token
          </button>{" "}
          to publish from here.
        </p>
        <div className="dialog-actions">
          <button type="button" className="secondary" onClick={onClose}>
            Cancel
          </button>
        </div>
      </>
    );
  }

  const publish = async () => {
    setBusy(true);
    setError(null);
    const res = await publishRepository({
      repoId: repo.id,
      accountId: options.accountId,
      organization: owner || null,
      name,
      description,
      isPrivate,
      ssh,
    });
    setBusy(false);
    if (res.status === "error") return setError(res.error);
    onClose();
    // The repository now has `origin`; pushing the branch shows progress in the toolbar.
    await refreshRepo(repo.id);
    await useSyncStore.getState().run("Publish");
  };

  const ready = name.trim() !== "" && !busy;
  return (
    <form
      className="form"
      onSubmit={(e) => {
        e.preventDefault();
        if (ready) void publish();
      }}
    >
      <label>
        Owner
        <select value={owner} onChange={(e) => setOwner(e.target.value)}>
          {owners.map((o) => (
            <option key={o} value={o}>
              {o || `${options.login} (you)`}
            </option>
          ))}
        </select>
      </label>
      {options.organizationsNeedToken && (
        <p className="muted">
          To publish to an organization,{" "}
          <button type="button" className="link" onClick={onUpdateToken}>
            update the token
          </button>{" "}
          so it can create repositories there.
        </p>
      )}
      <label>
        Name
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          autoFocus
          spellCheck={false}
        />
      </label>
      <label>
        Description (optional)
        <input value={description} onChange={(e) => setDescription(e.target.value)} />
      </label>
      <label className="radio">
        <input
          type="checkbox"
          checked={isPrivate}
          onChange={(e) => setIsPrivate(e.target.checked)}
        />
        Keep this repository private
      </label>
      <fieldset className="clone-protocol">
        <legend>Connect with</legend>
        <label>
          <input type="radio" name="protocol" checked={!ssh} onChange={() => setSsh(false)} />
          HTTPS (your Forgejo account)
        </label>
        <label>
          <input type="radio" name="protocol" checked={ssh} onChange={() => setSsh(true)} />
          SSH (your SSH key)
        </label>
      </fieldset>
      <p className="muted">
        Tenajlo creates the repository on the server, then publishes your current branch to it.
      </p>
      <InlineError error={error} />
      <div className="dialog-actions">
        <button type="button" className="secondary" onClick={onClose}>
          Cancel
        </button>
        <button type="submit" disabled={!ready}>
          {busy ? "Creating…" : "Publish repository"}
        </button>
      </div>
    </form>
  );
}
