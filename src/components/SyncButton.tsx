import { useEffect, useState } from "react";
import type { SyncRequest, SyncState } from "../bindings";
import { formatProgress } from "../lib/progress";
import { formatRelative } from "../lib/time";
import { useSyncStore } from "../stores/syncStore";
import { useUiStore } from "../stores/uiStore";

type Label = {
  request: SyncRequest;
  title: string;
  detail: string;
  /** A full sentence saying what the click does, ending with the git command it runs. */
  tooltip: string;
  badge?: string;
};

const commits = (n: number) => `${n} ${n === 1 ? "commit" : "commits"}`;

/**
 * Spec §8.1 (v1.4): the button says what it will do, not the git verb: Check for changes /
 * Get 3 commits / Send 2 commits / Publish branch. The tooltip names the git command. A
 * repository with no remote gets "Publish repository" instead, which opens a dialog.
 */
export function syncLabel(state: SyncState): Label | null {
  const { action, ahead, behind, lastFetched } = state;
  const checked =
    lastFetched === null
      ? "Never checked"
      : `Checked ${formatRelative(new Date(lastFetched * 1000).toISOString())}`;
  switch (action.type) {
    case "NoRemote":
      return null;
    case "Publish":
      return {
        request: "Publish",
        title: "Publish branch",
        detail: `Put this branch on ${action.remote}`,
        tooltip: `This branch is only on your computer. Click to put it on ${action.remote} (git push).`,
      };
    case "Pull":
      return {
        request: "Pull",
        title: `Get ${commits(behind)}`,
        detail: checked,
        badge: ahead > 0 ? `↓${behind} ↑${ahead}` : `↓${behind}`,
        tooltip:
          ahead > 0
            ? `${action.remote} has ${commits(behind)} you don't have, and you have ${commits(ahead)} it doesn't. Click to get theirs first; then you can send yours (git pull).`
            : `Your branch is ${commits(behind)} behind ${action.remote}. Click to get ${behind === 1 ? "it" : "them"} (git pull).`,
      };
    case "Push":
      return {
        request: "Push",
        title: `Send ${commits(ahead)}`,
        detail: checked,
        badge: `↑${ahead}`,
        tooltip: `You have ${commits(ahead)} that ${ahead === 1 ? "isn't" : "aren't"} on ${action.remote} yet. Click to send ${ahead === 1 ? "it" : "them"} (git push).`,
      };
    case "Fetch":
      return {
        request: "Fetch",
        title: "Check for changes",
        detail: checked,
        tooltip: `Ask ${action.remote} whether anyone has sent new commits. Nothing in your files changes (git fetch).`,
      };
  }
}

const RUNNING_TITLE: Record<SyncRequest, string> = {
  Fetch: "Checking…",
  Pull: "Getting commits…",
  PullMerge: "Merging…",
  Push: "Sending…",
  Publish: "Publishing…",
};

/** Re-renders once a minute, so "Checked 3 minutes ago" keeps up between fetches (with
 *  background checks off, nothing else would update it). */
function useMinuteTick() {
  const [, setTick] = useState(0);
  useEffect(() => {
    const timer = window.setInterval(() => setTick((t) => t + 1), 60_000);
    return () => window.clearInterval(timer);
  }, []);
}

export function SyncButton() {
  useMinuteTick();
  const state = useSyncStore((s) => s.state);
  const running = useSyncStore((s) => s.running);
  const run = useSyncStore((s) => s.run);
  const cancel = useSyncStore((s) => s.cancel);

  if (running) {
    const p = running.progress;
    return (
      <div className="sync-button running" role="status" aria-live="polite">
        <span className="toolbar-label">
          <small>{formatProgress(p)}</small>
          <span className="toolbar-value">{RUNNING_TITLE[running.request]}</span>
        </span>
        <progress max={100} value={p?.percent ?? undefined} aria-label="Progress" />
        <button type="button" className="secondary" onClick={cancel}>
          Cancel
        </button>
      </div>
    );
  }

  if (state?.action.type === "NoRemote") {
    return (
      <button
        type="button"
        className="toolbar-button sync-button"
        title="Put this repository on a Forgejo server"
        onClick={() => useUiStore.getState().openDialog("publishRepository")}
      >
        <span className="toolbar-label">
          <small>Not on a server yet</small>
          <span className="toolbar-value">Publish repository</span>
        </span>
      </button>
    );
  }

  const label = state && syncLabel(state);
  if (!label) return null;
  return (
    <button
      type="button"
      className="toolbar-button sync-button"
      title={label.tooltip}
      onClick={() => void run(label.request)}
    >
      <span className="toolbar-label">
        <small>{label.detail}</small>
        <span className="toolbar-value">{label.title}</span>
      </span>
      {label.badge && <span className="sync-badge">{label.badge}</span>}
    </button>
  );
}
