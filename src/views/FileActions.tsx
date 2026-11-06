import { useState } from "react";
import type { FileChange } from "../bindings";
import { discardChanges, ignoreExtension, ignoreFile } from "../api/changes";
import { openRepoFile, revealRepoFile } from "../api/merge";
import { trashName } from "../lib/platform";
import { useUiStore } from "../stores/uiStore";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { ContextMenu, type MenuItem } from "../components/ContextMenu";
import { useChangesStore } from "../stores/changesStore";
import { useLockStore } from "../stores/lockStore";

export type MenuPosition = { x: number; y: number };

/** Stable empty list: a fresh `[]` in a store selector would re-render forever. */
const NO_FILES: readonly FileChange[] = [];

/** `file` is null for the list header's menu. `extension`: what "Ignore all .ext files"
 *  would ignore (untracked files only). */
type MenuState = (MenuPosition & { file: FileChange | null; extension: string | null }) | null;

/** "Lock file" / "Unlock file" for a changed file, when the repository uses LFS locks. A
 *  lock held by someone else is shown but managed from the File locks dialog. */
function lockItems(file: FileChange): MenuItem[] {
  const locks = useLockStore.getState();
  if (!locks.available || file.kind === "Untracked" || file.submodule) return [];
  const lock = locks.locks.find((l) => l.path === file.path);
  if (!lock) {
    return [
      { label: "Lock file", onSelect: () => void locks.lock(file.path), disabled: locks.busy },
    ];
  }
  if (lock.ours) {
    return [
      {
        label: "Unlock file",
        onSelect: () => void locks.unlock(lock, false),
        disabled: locks.busy,
      },
    ];
  }
  return [
    {
      label: `Locked by ${lock.owner || "someone else"}…`,
      onSelect: () => useUiStore.getState().openDialog("locks"),
    },
  ];
}

/** Files "Discard all changes" applies to: conflicts and submodules are handled elsewhere. */
export function discardable(files: readonly FileChange[]): FileChange[] {
  return files.filter((f) => f.kind !== "Conflicted" && !f.submodule);
}

/** Right-click menu for a changed file (or the list header), plus the discard confirmation. */
export function useFileActions() {
  const mutate = useChangesStore((s) => s.mutate);
  const busy = useChangesStore((s) => s.busy);
  const files = useChangesStore((s) => s.status?.files ?? NO_FILES);
  const [menu, setMenu] = useState<MenuState>(null);
  const [confirm, setConfirm] = useState<string[] | null>(null);

  /** Opens the menu for `file` (or the whole list when null) at a screen position. */
  const openAt = async (file: FileChange | null, pos: MenuPosition) => {
    const extension = file?.kind === "Untracked" ? await ignoreExtension(file.path) : null;
    setMenu({ file, extension, ...pos });
  };

  const open = (file: FileChange | null, e: React.MouseEvent) => {
    e.preventDefault();
    void openAt(file, { x: e.clientX, y: e.clientY });
  };

  const discardAll = discardable(files);
  const discardAllItem: MenuItem = {
    label: "Discard all changes…",
    onSelect: () => setConfirm(discardAll.map((f) => f.path)),
    disabled: busy || discardAll.length === 0,
  };

  const items = ({ file, extension }: NonNullable<MenuState>): MenuItem[] => {
    if (!file) return [discardAllItem];
    const blocked = busy || file.kind === "Conflicted" || file.submodule;
    const repoId = useChangesStore.getState().repoId;
    const run = (action: typeof openRepoFile) => {
      if (!repoId) return;
      void action(repoId, file.path).then((res) => {
        if (res.status === "error") useUiStore.getState().showError(res.error);
      });
    };
    const list: MenuItem[] = [
      {
        label: "Open in editor",
        onSelect: () => run(openRepoFile),
        disabled: file.kind === "Deleted",
      },
      { label: "Show in folder", onSelect: () => run(revealRepoFile) },
      { label: "Discard changes…", onSelect: () => setConfirm([file.path]), disabled: blocked },
    ];
    if (file.kind === "Untracked") {
      list.push({
        label: "Ignore this file",
        onSelect: () => void mutate((id) => ignoreFile(id, file.path, false)),
        disabled: busy,
      });
      if (extension) {
        list.push({
          label: `Ignore all .${extension} files`,
          onSelect: () => void mutate((id) => ignoreFile(id, file.path, true)),
          disabled: busy,
        });
      }
    }
    list.push(...lockItems(file));
    if (discardAll.length > 1) list.push(discardAllItem);
    return list;
  };

  const many = (confirm?.length ?? 0) > 1;
  const ui = (
    <>
      {menu && (
        <ContextMenu x={menu.x} y={menu.y} items={items(menu)} onClose={() => setMenu(null)} />
      )}
      <ConfirmDialog
        open={confirm !== null}
        title={many ? "Discard all changes?" : "Discard changes?"}
        confirmLabel={many ? `Discard ${confirm?.length} files` : "Discard changes"}
        onCancel={() => setConfirm(null)}
        onConfirm={() => {
          const paths = confirm;
          setConfirm(null);
          if (paths) void mutate((id) => discardChanges(id, paths));
        }}
      >
        {many ? (
          <p>
            All changes to {confirm?.length} files will be undone. Their current versions will be
            moved to the {trashName()}, so you can still get them back.
          </p>
        ) : (
          <p>
            All changes to <code>{confirm?.[0]}</code> will be undone. The current version will be
            moved to the {trashName()}, so you can still get it back.
          </p>
        )}
      </ConfirmDialog>
    </>
  );

  return { open, openAt, ui };
}
