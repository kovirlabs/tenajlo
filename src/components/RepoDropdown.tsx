import { useState } from "react";
import { useRepoStore, useSelectedRepository } from "../stores/repoStore";
import { useUiStore } from "../stores/uiStore";
import { Popover } from "./Popover";

/** Toolbar "Current repository" dropdown with filter and "Add local…". */
export function RepoDropdown() {
  const selected = useSelectedRepository();
  const repositories = useRepoStore((s) => s.repositories);
  const select = useRepoStore((s) => s.select);
  const addLocal = useRepoStore((s) => s.addLocal);
  const [filter, setFilter] = useState("");
  const open = useUiStore((s) => s.repoPickerOpen);
  const setOpen = useUiStore((s) => s.setRepoPickerOpen);

  const needle = filter.trim().toLowerCase();
  const shown = repositories.filter((r) => r.name.toLowerCase().includes(needle));

  return (
    <Popover
      open={open}
      onOpenChange={setOpen}
      ariaLabel="Current repository"
      label={
        <span className="toolbar-label">
          <small>Current repository</small>
          {selected?.name ?? "None"}
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
  );
}
