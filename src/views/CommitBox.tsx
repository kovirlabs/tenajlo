import { useEffect, useState } from "react";
import type { Identity } from "../bindings";
import { commitChanges, getIdentity } from "../api/changes";
import { IdentityDialog } from "../components/IdentityDialog";
import { useChangesStore } from "../stores/changesStore";

/** Summary + description + "Commit to <branch>" (spec §8). Ctrl/Cmd+Enter commits. */
export function CommitBox({ repoId }: { repoId: string }) {
  const status = useChangesStore((s) => s.status);
  const busy = useChangesStore((s) => s.busy);
  const mutate = useChangesStore((s) => s.mutate);
  const [summary, setSummary] = useState("");
  const [description, setDescription] = useState("");
  const [identity, setIdentity] = useState<Identity | null>(null);
  const [askIdentity, setAskIdentity] = useState(false);

  const [identityVersion, setIdentityVersion] = useState(0);

  useEffect(() => {
    let live = true;
    void getIdentity(repoId).then((res) => {
      if (live && res.status === "ok") setIdentity(res.data);
    });
    return () => {
      live = false;
    };
  }, [repoId, identityVersion]);

  const anyStaged = status?.files.some((f) => f.staged !== "None") ?? false;
  const conflicts = status?.hasConflicts ?? false;
  const canCommit = !busy && !conflicts && anyStaged && summary.trim() !== "";
  const branch = status?.branch.name ?? "detached HEAD";
  const missingIdentity = identity !== null && (!identity.name || !identity.email);

  const commit = async () => {
    if (!canCommit) return;
    if (missingIdentity) return setAskIdentity(true);
    const ok = await mutate((id) => commitChanges(id, summary, description));
    if (ok) {
      setSummary("");
      setDescription("");
    }
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      void commit();
    }
  };

  return (
    <div className="commit-box">
      <input
        className="commit-summary-input"
        placeholder="Summary (required)"
        aria-label="Commit summary"
        value={summary}
        onChange={(e) => setSummary(e.target.value)}
        onKeyDown={onKeyDown}
      />
      <textarea
        placeholder="Description"
        aria-label="Commit description"
        rows={3}
        value={description}
        onChange={(e) => setDescription(e.target.value)}
        onKeyDown={onKeyDown}
      />
      {conflicts && <p className="commit-hint">Resolve conflicts before committing.</p>}
      {missingIdentity && (
        <p className="commit-hint">
          Git needs your name and email.{" "}
          <button type="button" className="link" onClick={() => setAskIdentity(true)}>
            Set them up
          </button>
        </p>
      )}
      <button type="button" disabled={!canCommit} onClick={() => void commit()}>
        Commit to <strong>{branch}</strong>
      </button>
      {identity && (
        <IdentityDialog
          open={askIdentity}
          initial={identity}
          onClose={() => setAskIdentity(false)}
          onSaved={() => {
            setAskIdentity(false);
            setIdentityVersion((v) => v + 1);
          }}
        />
      )}
    </div>
  );
}
