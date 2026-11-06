import { useState } from "react";
import type { CheckState, Commit, CommitChecks } from "../../bindings";
import { getCommitChecks } from "../../api/forgejo";
import { useAsyncEffect } from "../../hooks/useAsyncEffect";
import { useForgejoStore } from "../../stores/forgejoStore";

const LABEL: Record<CheckState, string> = {
  Success: "Checks passed",
  Failure: "Checks failed",
  Pending: "Checks running",
  Warning: "Checks passed with warnings",
};

const ICON: Record<CheckState, string> = {
  Success: "✓",
  Failure: "✕",
  Pending: "●",
  Warning: "!",
};

/** A pushed commit's checks and "View on Forgejo" (spec §8.1, v1.4). Hidden when unlinked. */
export function CommitChecksLine({ repoId, commit }: { repoId: string; commit: Commit }) {
  const linked = useForgejoStore((s) => s.repoId === repoId && s.link !== null);
  const open = useForgejoStore((s) => s.open);
  const [checks, setChecks] = useState<{ sha: string; checks: CommitChecks } | null>(null);

  useAsyncEffect(
    async (live) => {
      if (!linked || commit.unpushed) return;
      const res = await getCommitChecks(repoId, commit.sha);
      if (live() && res.status === "ok" && res.data)
        setChecks({ sha: commit.sha, checks: res.data });
    },
    [repoId, commit.sha, commit.unpushed, linked],
  );

  if (!linked || commit.unpushed) return null;
  const shown = checks?.sha === commit.sha ? checks.checks : null;
  const state = shown?.state ?? null;
  const failing = shown?.checks.filter((c) => c.state === "Failure").length ?? 0;
  return (
    <p className="commit-checks">
      {state && shown && (
        <span
          className={`check-state check-${state.toLowerCase()}`}
          title={shown.checks.map((c) => `${c.context}: ${c.description || c.state}`).join("\n")}
        >
          <span aria-hidden="true">{ICON[state]}</span> {LABEL[state]}
          {state === "Failure" &&
            shown.checks.length > 1 &&
            ` (${failing} of ${shown.checks.length})`}
        </span>
      )}
      <button
        type="button"
        className="link"
        onClick={() => void open({ type: "Commit", sha: commit.sha })}
      >
        View on Forgejo
      </button>
    </p>
  );
}
