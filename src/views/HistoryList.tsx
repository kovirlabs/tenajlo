import { useId, useState } from "react";
import type { Commit } from "../bindings";
import { ContextMenu } from "../components/ContextMenu";
import { VirtualList, rowId } from "../components/VirtualList";
import { listKeyHandler, menuPositionFor } from "../hooks/listKeys";
import { copyText } from "../lib/clipboard";
import { formatRelative } from "../lib/time";
import { useHistoryStore } from "../stores/historyStore";

const ROW_HEIGHT = 52;

type Menu = { commit: Commit; x: number; y: number } | null;

export function HistoryList() {
  const commits = useHistoryStore((s) => s.commits);
  const error = useHistoryStore((s) => s.error);
  const loading = useHistoryStore((s) => s.loading);
  const selectedSha = useHistoryStore((s) => s.selectedSha);
  const selectCommit = useHistoryStore((s) => s.selectCommit);
  const loadMore = useHistoryStore((s) => s.loadMore);
  const idPrefix = useId();
  const [menu, setMenu] = useState<Menu>(null);

  if (error && commits.length === 0) return <p className="pane-message">{error.message}</p>;
  if (commits.length === 0) {
    return <p className="pane-message muted">{loading ? "Loading…" : "No commits yet."}</p>;
  }

  const active = commits.findIndex((c) => c.sha === selectedSha);
  const onKeyDown = listKeyHandler({
    count: commits.length,
    active,
    onMove: (i) => {
      const c = commits[i];
      if (c) void selectCommit(c.sha);
    },
    onMenu: (i) => {
      const c = commits[i];
      if (c) setMenu({ commit: c, ...menuPositionFor(rowId(idPrefix, i)) });
    },
  });

  return (
    <>
      <VirtualList
        role="listbox"
        ariaLabel="Commits"
        items={commits}
        rowHeight={ROW_HEIGHT}
        onNearEnd={loadMore}
        idPrefix={idPrefix}
        activeIndex={active}
        onKeyDown={onKeyDown}
        renderRow={(c, _i, id) => (
          <CommitRow
            id={id}
            commit={c}
            selected={c.sha === selectedSha}
            onSelect={() => void selectCommit(c.sha)}
            onContextMenu={(e) => {
              e.preventDefault();
              setMenu({ commit: c, x: e.clientX, y: e.clientY });
            }}
          />
        )}
      />
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={[{ label: "Copy commit ID", onSelect: () => void copyText(menu.commit.sha) }]}
          onClose={() => setMenu(null)}
        />
      )}
    </>
  );
}

type RowProps = {
  id: string | undefined;
  commit: Commit;
  selected: boolean;
  onSelect: () => void;
  onContextMenu: (e: React.MouseEvent) => void;
};

function CommitRow({ id, commit, selected, onSelect, onContextMenu }: RowProps) {
  return (
    <div
      id={id}
      role="option"
      aria-selected={selected}
      className={selected ? "commit-row selected" : "commit-row"}
      onClick={onSelect}
      onContextMenu={onContextMenu}
    >
      <div className="commit-summary">{commit.summary || <em>(no message)</em>}</div>
      <div className="commit-meta muted">
        {commit.unpushed && (
          <span
            className="unpushed-badge"
            title="This commit is only on your computer. Push to share it."
          >
            Not pushed
          </span>
        )}
        {commit.authorName} · {formatRelative(commit.authorDate)}
      </div>
    </div>
  );
}
