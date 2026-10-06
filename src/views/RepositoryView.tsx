import { useEffect } from "react";
import type { Repository } from "../bindings";
import { useChangesStore } from "../stores/changesStore";
import { useUiStore, type Tab } from "../stores/uiStore";
import { ChangesList } from "./ChangesList";

const TABS: { id: Tab; label: string }[] = [
  { id: "changes", label: "Changes" },
  { id: "history", label: "History" },
];

/** Main view for an open repository: sidebar tabs + detail pane. */
export function RepositoryView({ repo }: { repo: Repository }) {
  const tab = useUiStore((s) => s.tab);
  const setTab = useUiStore((s) => s.setTab);
  const refreshChanges = useChangesStore((s) => s.refresh);

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
      <section className="detail" aria-label="Details" />
    </div>
  );
}
