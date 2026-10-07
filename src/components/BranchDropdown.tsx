import { useState } from "react";
import type { Branch, LocalChanges } from "../bindings";
import { switchBranch } from "../api/branches";
import { useBranchStore } from "../stores/branchStore";
import { useChangesStore } from "../stores/changesStore";
import { useUiStore } from "../stores/uiStore";
import { ContextMenu } from "./ContextMenu";
import { Popover } from "./Popover";
import { DeleteBranchDialog } from "./branches/DeleteBranchDialog";
import { SwitchDialog } from "./branches/SwitchDialog";

/** Toolbar "Current branch" dropdown: switch, create, delete. */
export function BranchDropdown() {
  const branches = useBranchStore((s) => s.branches);
  const head = useChangesStore((s) => s.status?.branch);
  const hasChanges = useChangesStore((s) => (s.status?.files.length ?? 0) > 0);
  const busy = useChangesStore((s) => s.busy);
  const mutate = useChangesStore((s) => s.mutate);
  const openDialog = useUiStore((s) => s.openDialog);
  const [filter, setFilter] = useState("");
  const [pendingSwitch, setPendingSwitch] = useState<string | null>(null);
  const [toDelete, setToDelete] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ name: string; x: number; y: number } | null>(null);

  // Status knows the branch name even before the first commit, when no refs exist yet.
  const current = head?.name ?? (head?.tip ? "Detached HEAD" : "—");
  const needle = filter.trim().toLowerCase();
  const match = (b: Branch) => b.name.toLowerCase().includes(needle);
  const others = branches?.local.filter((b) => !b.isCurrent && match(b)) ?? [];
  const remote = branches?.remoteOnly.filter(match) ?? [];

  const doSwitch = (name: string, choice: LocalChanges) =>
    void mutate((id) => switchBranch(id, name, choice));

  const pick = (name: string, close: () => void) => {
    close();
    // Leaving changes behind needs a branch to leave them on.
    if (hasChanges && head?.name) setPendingSwitch(name);
    else doSwitch(name, "Bring");
  };

  return (
    <>
      <Popover
        ariaLabel="Current branch"
        label={
          <span className="toolbar-label">
            <small>Current branch</small>
            {current}
          </span>
        }
      >
        {(close) => (
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
              <Section title="Current branch">
                <li className="picker-item current">{current}</li>
              </Section>
              <Section title="Other branches">
                {others.map((b) => (
                  <BranchItem
                    key={b.name}
                    name={b.name}
                    disabled={busy}
                    onPick={() => pick(b.name, close)}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      setMenu({ name: b.name, x: e.clientX, y: e.clientY });
                    }}
                  />
                ))}
              </Section>
              <Section title="Remote branches">
                {remote.map((b) => (
                  <BranchItem
                    key={b.name}
                    name={b.name}
                    disabled={busy}
                    onPick={() => pick(b.name, close)}
                  />
                ))}
              </Section>
            </div>
            <div className="picker-footer">
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  close();
                  openDialog("newBranch");
                }}
              >
                New branch…
              </button>
            </div>
          </div>
        )}
      </Popover>
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          items={[{ label: "Delete…", onSelect: () => setToDelete(menu.name), disabled: busy }]}
        />
      )}
      <SwitchDialog
        target={pendingSwitch}
        current={current}
        onCancel={() => setPendingSwitch(null)}
        onSwitch={(choice) => {
          const name = pendingSwitch;
          setPendingSwitch(null);
          if (name) doSwitch(name, choice);
        }}
      />
      <DeleteBranchDialog name={toDelete} onClose={() => setToDelete(null)} />
    </>
  );
}

function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode[] | React.ReactNode;
}) {
  if (Array.isArray(children) && children.length === 0) return null;
  return (
    <section>
      <h3 className="picker-section">{title}</h3>
      <ul className="picker-list-plain">{children}</ul>
    </section>
  );
}

type ItemProps = {
  name: string;
  disabled: boolean;
  onPick: () => void;
  onContextMenu?: (e: React.MouseEvent) => void;
};

function BranchItem({ name, disabled, onPick, onContextMenu }: ItemProps) {
  return (
    <li>
      <button
        type="button"
        className="picker-item"
        disabled={disabled}
        onClick={onPick}
        onContextMenu={onContextMenu}
      >
        {name}
      </button>
    </li>
  );
}
