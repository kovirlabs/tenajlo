import { useEffect, useState } from "react";
import type { Account } from "../../bindings";
import { useCloneStore } from "../../stores/cloneStore";
import { useUiStore } from "../../stores/uiStore";
import { InlineError } from "../InlineError";

type Props = {
  accounts: Account[];
  selectedUrl: string;
  onSelect: (cloneUrl: string) => void;
};

/** "Your Forgejo repositories" tab: pick a repository from a signed-in account. */
export function ForgejoRepoList({ accounts, selectedUrl, onSelect }: Props) {
  const [accountId, setAccountId] = useState(accounts[0]?.id ?? "");
  const [filter, setFilter] = useState("");
  const account = accounts.find((a) => a.id === accountId) ?? accounts[0] ?? null;
  const list = useCloneStore((s) => (account ? s.lists[account.id] : undefined));
  const loading = useCloneStore((s) => account !== null && s.loadingAccount === account.id);
  const error = useCloneStore((s) => s.listError);
  const loadRepos = useCloneStore((s) => s.loadRepos);

  useEffect(() => {
    if (account && !account.needsSignIn) void loadRepos(account.id);
  }, [account, loadRepos]);

  if (!account) {
    return (
      <div className="clone-empty">
        <p>Sign in to your Forgejo server to see the repositories you can clone.</p>
        <button type="button" onClick={() => useUiStore.getState().openDialog("accounts")}>
          Sign in to Forgejo…
        </button>
      </div>
    );
  }

  if (account.needsSignIn) {
    return (
      <div className="clone-empty">
        <p>Your sign-in for {account.baseUrl} has stopped working.</p>
        <button type="button" onClick={() => useUiStore.getState().signInAgain(account.id)}>
          Sign in again
        </button>
      </div>
    );
  }

  const needle = filter.trim().toLowerCase();
  const repos = (list?.repos ?? []).filter((r) => r.fullName.toLowerCase().includes(needle));

  return (
    <div className="form">
      <div className="clone-list-controls">
        {accounts.length > 1 && (
          <select
            aria-label="Account"
            value={account.id}
            onChange={(e) => setAccountId(e.target.value)}
          >
            {accounts.map((a) => (
              <option key={a.id} value={a.id}>
                {a.login} on {a.baseUrl}
              </option>
            ))}
          </select>
        )}
        <input
          className="clone-filter"
          placeholder="Filter"
          aria-label="Filter repositories"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          autoFocus
        />
        <button
          type="button"
          className="secondary"
          disabled={loading}
          onClick={() => void loadRepos(account.id, true)}
        >
          Refresh
        </button>
      </div>
      <InlineError error={error} />
      <ul className="clone-repo-list" role="listbox" aria-label="Repositories">
        {loading && !list && <li className="muted picker-empty">Loading repositories…</li>}
        {list && repos.length === 0 && <li className="muted picker-empty">No repositories</li>}
        {repos.map((r) => (
          <li key={r.fullName} role="option" aria-selected={r.cloneUrl === selectedUrl}>
            <button
              type="button"
              className={r.cloneUrl === selectedUrl ? "picker-item current" : "picker-item"}
              onClick={() => onSelect(r.cloneUrl)}
            >
              <span>
                {r.owner}/<strong>{r.name}</strong>
                {r.private && <span className="badge">Private</span>}
                {r.archived && <span className="badge">Archived</span>}
              </span>
              {r.description && <span className="muted clone-repo-desc">{r.description}</span>}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
