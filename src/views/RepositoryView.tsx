import { useEffect } from "react";
import type { Repository } from "../bindings";
import { useChangesStore } from "../stores/changesStore";
import { useUiStore, type Tab } from "../stores/uiStore";
import { getWorkingDiff } from "../api/diff";
import { ChangesList } from "./ChangesList";
import { DiffPane } from "./DiffPane";

const TABS: { id: Tab; label: string }[] = [
  { id: "changes", label: "Changes" },
  { id: "history", label: "History" },
];

/** Main view for an open repository: sidebar tabs + detail pane. */
export function RepositoryView({ repo }: { repo: Repository }) {
  const tab = useUiStore((s) => s.tab);
  const setTab = useUiStore((s) => s.setTab);
  const refreshChanges = useChangesStore((s) => s.refresh);
  const selectedPath = useChangesStore((s) => s.selectedPath);
  const status = useChangesStore((s) => s.status);

  useEffect(() => {
    void refreshChanges(repo.id);
  }, [repo.id, refreshChanges]);

  return (
    <div className="repo-view">
      <aside className="sidebar">
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
        {tab === "changes" ? <ChangesList /> : <p className="pane-message muted">History</p>}
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
          />
        )}
      </section>
    </div>
  );
}
