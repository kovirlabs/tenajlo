import { useChangesStore } from "../../stores/changesStore";
import { useForgejoStore } from "../../stores/forgejoStore";

/** "#12" next to the branch name when it has an open pull request. */
export function PullRequestPill() {
  const pull = useForgejoStore((s) => s.link?.pull ?? null);
  if (!pull) return null;
  return (
    <span className="pr-pill" title={`Pull request #${pull.number}: ${pull.title}`}>
      #{pull.number}
    </span>
  );
}

/**
 * The branch dropdown's Forgejo section (spec §8.1, v1.4): open the branch's pull request, or
 * create one once the branch is on the server.
 */
export function BranchPullRequestSection({ close }: { close: () => void }) {
  const link = useForgejoStore((s) => s.link);
  const branch = useForgejoStore((s) => s.branch);
  const open = useForgejoStore((s) => s.open);
  const pushed = useChangesStore((s) => {
    const b = s.status?.branch;
    return Boolean(b?.upstream && !b.upstreamGone);
  });
  if (!link || !branch) return null;

  let item: React.ReactNode = null;
  if (link.pull) {
    const { number, title } = link.pull;
    item = (
      <button
        type="button"
        className="picker-item pr-item"
        title={`Open pull request #${number} on Forgejo`}
        onClick={() => {
          close();
          void open({ type: "PullRequest", number });
        }}
      >
        <span className="pr-pill">#{number}</span> {title}
      </button>
    );
  } else if (pushed && branch !== link.defaultBranch) {
    item = (
      <button
        type="button"
        className="picker-item"
        title={`Compare ${branch} with ${link.defaultBranch} on Forgejo`}
        onClick={() => {
          close();
          void open({ type: "NewPullRequest", branch });
        }}
      >
        Create pull request…
      </button>
    );
  }
  if (!item) return null;
  return (
    <section>
      <h3 className="picker-section">Pull request</h3>
      <ul className="picker-list-plain">
        <li>{item}</li>
      </ul>
    </section>
  );
}
