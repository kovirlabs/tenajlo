import { useState } from "react";
import type { Repository } from "../bindings";
import { useRepoStore, useSelectedRepository } from "../stores/repoStore";
import { useUiStore } from "../stores/uiStore";
import { ConfirmDialog } from "./ConfirmDialog";
import { ContextMenu } from "./ContextMenu";
import { Popover } from "./Popover";

type Menu = { repo: Repository; x: number; y: number } | null;

/** Toolbar "Current repository" dropdown with filter, "Add local…", and a right-click
 *  "Remove from list…" per repository. */
export function RepoDropdown() {
  const selected = useSelectedRepository();
  const repositories = useRepoStore((s) => s.repositories);
  const select = useRepoStore((s) => s.select);
  const addLocal = useRepoStore((s) => s.addLocal);
  const remove = useRepoStore((s) => s.remove);
  const [filter, setFilter] = useState("");
  const open = useUiStore((s) => s.repoPickerOpen);
  const setRepoPickerOpen = useUiStore((s) => s.setRepoPickerOpen);
  const [menu, setMenu] = useState<Menu>(null);
  const [confirmRemove, setConfirmRemove] = useState<Repository | null>(null);

  // Each opening starts with an empty filter.
  const setOpen = (next: boolean) => {
    if (!next) setFilter("");
    setRepoPickerOpen(next);
  };

  const needle = filter.trim().toLowerCase();
  const shown = repositories.filter((r) => r.name.toLowerCase().includes(needle));

  const openMenu = (repo: Repository, x: number, y: number) => {
    setOpen(false);
    setMenu({ repo, x, y });
  };

  return (
    <>
      <Popover
        open={open}
        onOpenChange={setOpen}
        ariaLabel="Current repository"
        label={
          <span className="toolbar-label">
            <small>Current repository</small>
            <span className="toolbar-value" title={selected?.name}>
              {selected?.name ?? "None"}
            </span>
          </span>
        }
      >
        {(close) => (
          <div className="picker">
            <input
              className="picker-filter"
              placeholder="Filter"
              aria-label="Filter repositories"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              onKeyDown={(e) => {
                const first = shown[0];
                if (e.key === "Enter" && first) {
                  e.preventDefault();
                  close();
                  void select(first.id);
                }
              }}
              autoFocus
            />
            <ul className="picker-list">
              {shown.map((r) => (
                <li key={r.id}>
                  <button
                    type="button"
                    className={r.id === selected?.id ? "picker-item current" : "picker-item"}
                    title={r.path}
                    onClick={() => {
                      close();
                      void select(r.id);
                    }}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      openMenu(r, e.clientX, e.clientY);
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey)) {
                        e.preventDefault();
                        const rect = e.currentTarget.getBoundingClientRect();
                        openMenu(r, rect.left + 24, rect.bottom);
                      }
                    }}
                  >
                    {r.name}
                    {r.missing && <span className="muted"> (missing)</span>}
                  </button>
                </li>
              ))}
              {shown.length === 0 && <li className="muted picker-empty">No repositories</li>}
            </ul>
            <div className="picker-footer">
              <button
                type="button"
                className="secondary"
                onClick={() => {
                  close();
                  useUiStore.getState().openSettings("accounts");
                }}
              >
                Accounts…
              </button>
              <button
                type="button"
                className="secondary"
                onClick={() => {
                  close();
                  useUiStore.getState().openDialog("clone");
                }}
              >
                Clone…
              </button>
              <button
                type="button"
                className="secondary"
                onClick={() => {
                  close();
                  useUiStore.getState().openDialog("newRepository");
                }}
              >
                New…
              </button>
              <button
                type="button"
                onClick={() => {
                  close();
                  void addLocal();
                }}
              >
                Add local…
              </button>
            </div>
          </div>
        )}
      </Popover>
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={[{ label: "Remove from list…", onSelect: () => setConfirmRemove(menu.repo) }]}
          onClose={() => setMenu(null)}
        />
      )}
      <ConfirmDialog
        open={confirmRemove !== null}
        title="Remove from list?"
        confirmLabel="Remove"
        onCancel={() => setConfirmRemove(null)}
        onConfirm={() => {
          const repo = confirmRemove;
          setConfirmRemove(null);
          if (repo) void remove(repo.id);
        }}
      >
        <p>
          <strong>{confirmRemove?.name}</strong> will be removed from Tenajlo&apos;s list. The
          folder and its files stay on your computer, and you can add it again later.
        </p>
      </ConfirmDialog>
    </>
  );
}
