import type { Repository } from "../bindings";
import { useChangesStore } from "../stores/changesStore";
import { useUiStore, type Tab } from "../stores/uiStore";
import { getWorkingDiff } from "../api/diff";
import { useRepoRefresh } from "../hooks/useRepoRefresh";
import { useBackgroundFetch } from "../hooks/useBackgroundFetch";
import { ConflictBanner } from "../components/conflicts/ConflictBanner";
import { LfsBanner } from "../components/LfsBanner";
import { ChangesList } from "./ChangesList";
import { CommitBox } from "./CommitBox";
import { CommitDetail } from "./CommitDetail";
import { DiffPane } from "./DiffPane";
import { HistoryList } from "./HistoryList";

const TABS: { id: Tab; label: string }[] = [
  { id: "changes", label: "Changes" },
  { id: "history", label: "History" },
];

/** Main view for an open repository: sidebar tabs + detail pane. */
export function RepositoryView({ repo }: { repo: Repository }) {
  const tab = useUiStore((s) => s.tab);
  const setTab = useUiStore((s) => s.setTab);
  const selectedPath = useChangesStore((s) => s.selectedPath);
  const status = useChangesStore((s) => s.status);
  const busy = useChangesStore((s) => s.busy);
  const setLinesStaged = useChangesStore((s) => s.setLinesStaged);
  useRepoRefresh(repo.id);
  useBackgroundFetch(repo.id);

  return (
    <div className="repo-view">
      <aside className="sidebar">
        <LfsBanner repoId={repo.id} />
        <ConflictBanner repoId={repo.id} />
        <div role="tablist" className="tabs">
          {TABS.map((t) => (
            <button
              key={t.id}
              type="button"
              role="tab"
              aria-selected={tab === t.id}
              className="tab"
              onClick={() => setTab(t.id)}
            >
              {t.label}
            </button>
          ))}
        </div>
        {tab === "changes" ? (
          <>
            <ChangesList />
            <CommitBox key={repo.id} repoId={repo.id} />
          </>
        ) : (
          <HistoryList />
        )}
      </aside>
      <section className="detail" aria-label="Details">
        {tab === "changes" && (
          <DiffPane
            source={
              selectedPath === null
                ? null
                : {
                    key: `work:${repo.id}:${selectedPath}`,
                    title: selectedPath,
                    fetch: () => getWorkingDiff(repo.id, selectedPath),
                  }
            }
            version={status}
            empty="No changes to show."
            busy={busy}
            onStageLines={(token, lines, staged) => {
              if (selectedPath !== null) void setLinesStaged(selectedPath, token, lines, staged);
            }}
          />
        )}
        {tab === "history" && <CommitDetail repoId={repo.id} />}
      </section>
    </div>
  );
}
