import type { SyncRequest, SyncState } from "../bindings";
import { formatRelative } from "../lib/time";
import { useSyncStore } from "../stores/syncStore";

type Label = { request: SyncRequest; title: string; detail: string; badge?: string };

/** Spec §8.1: Publish branch / Fetch origin / Pull origin (↓3) / Push origin (↑2). */
export function describe(state: SyncState): Label | null {
  const { action, ahead, behind, lastFetched } = state;
  const fetched =
    lastFetched === null
      ? "Never fetched"
      : `Last fetched ${formatRelative(new Date(lastFetched * 1000).toISOString())}`;
  switch (action.type) {
    case "NoRemote":
      return null;
    case "Publish":
      return {
        request: "Publish",
        title: "Publish branch",
        detail: `Put this branch on ${action.remote}`,
      };
    case "Pull":
      return {
        request: "Pull",
        title: `Pull ${action.remote}`,
        detail: fetched,
        badge: ahead > 0 ? `↓${behind} ↑${ahead}` : `↓${behind}`,
      };
    case "Push":
      return {
        request: "Push",
        title: `Push ${action.remote}`,
        detail: fetched,
        badge: `↑${ahead}`,
      };
    case "Fetch":
      return { request: "Fetch", title: `Fetch ${action.remote}`, detail: fetched };
  }
}

const RUNNING_TITLE: Record<SyncRequest, string> = {
  Fetch: "Fetching…",
  Pull: "Pulling…",
  PullMerge: "Merging…",
  Push: "Pushing…",
  Publish: "Publishing…",
};

export function SyncButton() {
  const state = useSyncStore((s) => s.state);
  const running = useSyncStore((s) => s.running);
  const run = useSyncStore((s) => s.run);
  const cancel = useSyncStore((s) => s.cancel);

  if (running) {
    const p = running.progress;
    return (
      <div className="sync-button running" role="status" aria-live="polite">
        <span className="toolbar-label">
          <small>
            {p ? `${p.phase}${p.percent !== null ? ` ${p.percent}%` : ""}` : "Starting…"}
          </small>
          {RUNNING_TITLE[running.request]}
        </span>
        <progress max={100} value={p?.percent ?? undefined} aria-label="Progress" />
        <button type="button" className="secondary" onClick={cancel}>
          Cancel
        </button>
      </div>
    );
  }

  const label = state && describe(state);
  if (!label) return null;
  return (
    <button
      type="button"
      className="toolbar-button sync-button"
      title={label.detail}
      onClick={() => void run(label.request)}
    >
      <span className="toolbar-label">
        <small>{label.detail}</small>
        {label.title}
      </span>
      {label.badge && <span className="sync-badge">{label.badge}</span>}
    </button>
  );
}
