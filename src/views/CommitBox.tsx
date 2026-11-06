import { useState } from "react";
import type { Identity, SyncRequest } from "../bindings";
import { commitChanges, getIdentity, undoCommit } from "../api/changes";
import { IdentityDialog } from "../components/IdentityDialog";
import { useChangesStore } from "../stores/changesStore";
import { useMerging } from "../stores/conflictStore";
import { useDraft, useDraftStore } from "../stores/draftStore";
import { useSyncStore } from "../stores/syncStore";
import { useLockStore } from "../stores/lockStore";
import { useAsyncEffect } from "../hooks/useAsyncEffect";
import { formatRelative } from "../lib/time";

type LastCommit = { sha: string; summary: string; branch: string | null; at: string };

/** Summary + description + "Commit to <branch>" (spec §8). Ctrl/Cmd+Enter commits. The
 *  message is a per-repository draft, so switching repositories doesn't lose it. */
/** What a queued change is waiting for, in plain words. */
const WAITING_LABEL: Record<SyncRequest, string> = {
  Fetch: "check for changes",
  Pull: "download",
  PullMerge: "merge",
  Push: "upload",
  Publish: "publish",
};

/** A warning when an included file is locked by someone else (their lock blocks the push). */
function useLockedByOthers(): string | null {
  const files = useChangesStore((s) => s.status?.files);
  const locks = useLockStore((s) => (s.available ? s.locks : null));
  if (!files || !locks) return null;
  const included = new Set(files.filter((f) => f.staged !== "None").map((f) => f.path));
  const theirs = locks.filter((l) => !l.ours && included.has(l.path));
  const first = theirs[0];
  if (!first) return null;
  const who = first.owner || "someone else";
  return theirs.length === 1
    ? `${first.path} is locked by ${who}. The push will be refused until they unlock it.`
    : `${theirs.length} included files are locked by others (${first.path} by ${who}). The push will be refused until they're unlocked.`;
}

export function CommitBox({ repoId }: { repoId: string }) {
  const status = useChangesStore((s) => s.status);
  const busy = useChangesStore((s) => s.busy);
  const mutate = useChangesStore((s) => s.mutate);
  const draft = useDraft(repoId);
  const updateDraft = useDraftStore((s) => s.update);
  const clearDraft = useDraftStore((s) => s.clear);
  const { description, summaryTouched } = draft;
  // Until the user types, a merge's summary is prefilled from git's merge message.
  const { merging, otherOperation, summary: mergeSummary } = useMerging();
  const summary = !summaryTouched && merging && mergeSummary ? mergeSummary : draft.summary;
  const waitingFor = useSyncStore((s) =>
    s.running ? s.running.request : s.backgroundFetching ? "Fetch" : null,
  );
  const [identity, setIdentity] = useState<Identity | null>(null);
  const [askIdentity, setAskIdentity] = useState(false);
  const [lastCommit, setLastCommit] = useState<LastCommit | null>(null);

  const [identityVersion, setIdentityVersion] = useState(0);

  useAsyncEffect(
    async (live) => {
      const res = await getIdentity(repoId);
      if (live() && res.status === "ok") setIdentity(res.data);
    },
    [repoId, identityVersion],
  );

  const anyStaged = status?.files.some((f) => f.staged !== "None") ?? false;
  const lockedByOthers = useLockedByOthers();
  const conflicts = status?.hasConflicts ?? false;
  const canCommit =
    !busy && !conflicts && !otherOperation && (anyStaged || merging) && summary.trim() !== "";
  const branch = status?.branch.name ?? "detached HEAD";
  const missingIdentity = identity !== null && (!identity.name || !identity.email);

  const commit = async () => {
    if (!canCommit) return;
    if (missingIdentity) return setAskIdentity(true);
    const committedSummary = summary.trim();
    const committedBranch = status?.branch.name ?? null;
    const res = await mutate((id) => commitChanges(id, summary, description));
    if (res?.status === "ok") {
      setLastCommit({
        sha: res.data,
        summary: committedSummary,
        branch: committedBranch,
        at: new Date().toISOString(),
      });
      clearDraft(repoId);
    }
  };

  const undo = async () => {
    if (!lastCommit) return;
    const { sha } = lastCommit;
    const res = await mutate((id) => undoCommit(id, sha));
    if (res?.status === "ok") {
      updateDraft(repoId, {
        summary: res.data.summary,
        summaryTouched: true,
        description: res.data.description,
      });
      setLastCommit(null);
    }
  };

  // Only offer Undo while our commit is still the tip of the branch it was made on, and not on
  // the server yet (with an upstream, nothing ahead means it was pushed).
  const tip = status?.branch;
  const canUndo =
    lastCommit !== null &&
    tip?.tip === lastCommit.sha &&
    tip.name === lastCommit.branch &&
    !(tip.upstream && tip.ahead === 0);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      void commit();
    }
  };

  return (
    <div className="commit-box">
      {canUndo && (
        <div className="undo-bar" role="status">
          <span>
            Committed {formatRelative(lastCommit.at)} · <strong>{lastCommit.summary}</strong>
          </span>
          <button type="button" className="secondary" disabled={busy} onClick={() => void undo()}>
            Undo
          </button>
        </div>
      )}
      <input
        className="commit-summary-input"
        placeholder="Summary (required)"
        aria-label="Commit summary"
        value={summary}
        onChange={(e) => updateDraft(repoId, { summary: e.target.value, summaryTouched: true })}
        onKeyDown={onKeyDown}
      />
      <textarea
        placeholder="Description"
        aria-label="Commit description"
        rows={3}
        value={description}
        onChange={(e) => updateDraft(repoId, { description: e.target.value })}
        onKeyDown={onKeyDown}
      />
      {conflicts && <p className="commit-hint">Resolve conflicts before committing.</p>}
      {lockedByOthers && (
        <p className="commit-hint" role="status">
          {lockedByOthers}
        </p>
      )}
      {busy && waitingFor && (
        <p className="commit-hint muted" role="status">
          Waiting for the {WAITING_LABEL[waitingFor]} to finish…
        </p>
      )}
      {missingIdentity && (
        <p className="commit-hint">
          Git needs your name and email.{" "}
          <button type="button" className="link" onClick={() => setAskIdentity(true)}>
            Set them up
          </button>
        </p>
      )}
      <button
        type="button"
        className="commit-button"
        title={`${merging ? "Commit merge to" : "Commit to"} ${branch}`}
        disabled={!canCommit}
        onClick={() => void commit()}
      >
        {merging ? "Commit merge to " : "Commit to "}
        <strong>{branch}</strong>
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
