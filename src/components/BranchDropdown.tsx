import { useState } from "react";
import type { Branch } from "../bindings";
import { useBranchStore } from "../stores/branchStore";
import { useChangesStore } from "../stores/changesStore";
import { Popover } from "./Popover";

/** Toolbar "Current branch" dropdown. Switching branches arrives in M2. */
export function BranchDropdown() {
  const branches = useBranchStore((s) => s.branches);
  const head = useChangesStore((s) => s.status?.branch);
  const [filter, setFilter] = useState("");

  // Status knows the branch name even before the first commit, when no refs exist yet.
  const current = head?.name ?? (head?.tip ? "Detached HEAD" : "—");
  const needle = filter.trim().toLowerCase();
  const match = (b: Branch) => b.name.toLowerCase().includes(needle);
  const others = branches?.local.filter((b) => !b.isCurrent && match(b)) ?? [];
  const remote = branches?.remoteOnly.filter(match) ?? [];

  return (
    <Popover
      ariaLabel="Current branch"
      label={
        <span className="toolbar-label">
          <small>Current branch</small>
          {current}
        </span>
      }
    >
      {() => (
        <div className="picker">
          <input
            className="picker-filter"
            placeholder="Filter"
            aria-label="Filter branches"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            autoFocus
          />
          <div className="picker-list">
            <Section title="Current branch" items={[current]} current />
            <Section title="Other branches" items={others.map((b) => b.name)} />
            <Section title="Remote branches" items={remote.map((b) => b.name)} />
          </div>
          <p className="picker-note muted">Switching branches is coming in the next update.</p>
        </div>
      )}
    </Popover>
  );
}

function Section({ title, items, current }: { title: string; items: string[]; current?: boolean }) {
  if (items.length === 0) return null;
  return (
    <section>
      <h3 className="picker-section">{title}</h3>
      <ul className="picker-list-plain">
        {items.map((name) => (
          <li key={name} className={current ? "picker-item current" : "picker-item"}>
            {name}
          </li>
        ))}
      </ul>
    </section>
  );
}
