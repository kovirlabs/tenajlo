import type { Commit } from "../bindings";
import { VirtualList } from "../components/VirtualList";
import { formatRelative } from "../lib/time";
import { useHistoryStore } from "../stores/historyStore";

const ROW_HEIGHT = 52;

export function HistoryList() {
  const commits = useHistoryStore((s) => s.commits);
  const error = useHistoryStore((s) => s.error);
  const loading = useHistoryStore((s) => s.loading);
  const selectedSha = useHistoryStore((s) => s.selectedSha);
  const selectCommit = useHistoryStore((s) => s.selectCommit);
  const loadMore = useHistoryStore((s) => s.loadMore);

  if (error && commits.length === 0) return <p className="pane-message">{error.message}</p>;
  if (commits.length === 0) {
    return <p className="pane-message muted">{loading ? "Loading…" : "No commits yet."}</p>;
  }

  return (
    <VirtualList
      ariaLabel="Commits"
      items={commits}
      rowHeight={ROW_HEIGHT}
      onNearEnd={() => void loadMore()}
      renderRow={(c) => (
        <CommitRow
          commit={c}
          selected={c.sha === selectedSha}
          onSelect={() => void selectCommit(c.sha)}
        />
      )}
    />
  );
}

type RowProps = { commit: Commit; selected: boolean; onSelect: () => void };

function CommitRow({ commit, selected, onSelect }: RowProps) {
  return (
    <div
      role="option"
      aria-selected={selected}
      className={selected ? "commit-row selected" : "commit-row"}
      onClick={onSelect}
    >
      <div className="commit-summary">{commit.summary || <em>(no message)</em>}</div>
      <div className="commit-meta muted">
        {commit.authorName} · {formatRelative(commit.authorDate)}
      </div>
    </div>
  );
}
