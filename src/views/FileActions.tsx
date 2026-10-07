import { useState } from "react";
import type { FileChange } from "../bindings";
import { discardChanges, ignoreFile } from "../api/changes";
import { openRepoFile, revealRepoFile } from "../api/merge";
import { useUiStore } from "../stores/uiStore";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { ContextMenu, type MenuItem } from "../components/ContextMenu";
import { useChangesStore } from "../stores/changesStore";

type MenuState = { file: FileChange; x: number; y: number } | null;

/** Right-click menu for a changed file, plus the discard confirmation it can open. */
export function useFileActions() {
  const mutate = useChangesStore((s) => s.mutate);
  const busy = useChangesStore((s) => s.busy);
  const [menu, setMenu] = useState<MenuState>(null);
  const [confirm, setConfirm] = useState<FileChange | null>(null);

  const open = (file: FileChange, e: React.MouseEvent) => {
    e.preventDefault();
    setMenu({ file, x: e.clientX, y: e.clientY });
  };

  const items = (file: FileChange): MenuItem[] => {
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
      { label: "Discard changes…", onSelect: () => setConfirm(file), disabled: blocked },
    ];
    if (file.kind === "Untracked") {
      list.push({
        label: "Ignore this file",
        onSelect: () => void mutate((id) => ignoreFile(id, file.path, false)),
        disabled: busy,
      });
      const ext = file.path.match(/[^/]\.([\w-]+)$/)?.[1];
      if (ext) {
        list.push({
          label: `Ignore all .${ext} files`,
          onSelect: () => void mutate((id) => ignoreFile(id, file.path, true)),
          disabled: busy,
        });
      }
    }
    return list;
  };

  const ui = (
    <>
      {menu && (
        <ContextMenu x={menu.x} y={menu.y} items={items(menu.file)} onClose={() => setMenu(null)} />
      )}
      <ConfirmDialog
        open={confirm !== null}
        title="Discard changes?"
        confirmLabel="Discard changes"
        onCancel={() => setConfirm(null)}
        onConfirm={() => {
          const file = confirm;
          setConfirm(null);
          if (file) void mutate((id) => discardChanges(id, [file.path]));
        }}
      >
        <p>
          All changes to <code>{confirm?.path}</code> will be undone. The current version will be
          moved to the Trash, so you can still get it back.
        </p>
      </ConfirmDialog>
    </>
  );

  return { open, ui };
}
