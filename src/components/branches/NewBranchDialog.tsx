import { useState } from "react";
import { createBranch, previewBranchName } from "../../api/branches";
import { useBranchStore } from "../../stores/branchStore";
import { useChangesStore } from "../../stores/changesStore";
import { Modal } from "../Modal";
import { useAsyncEffect } from "../../hooks/useAsyncEffect";
import { useDialog } from "../../hooks/useDialog";

/** "New branch…" — created from the current commit; local changes come along. */
export function NewBranchDialog() {
  const { open, close } = useDialog("newBranch");
  const mutate = useChangesStore((s) => s.mutate);
  const from = useChangesStore((s) => s.status?.branch.name);
  const [name, setName] = useState("");
  const [preview, setPreview] = useState("");
  // The existing branch `name` would clash with: the same name, or one that differs only in
  // capitals (Windows and macOS treat those as the same branch; Rust refuses them too).
  const takenBy = (branch: string) => {
    const folded = branch.toLowerCase();
    return (
      useBranchStore.getState().branches?.local.find((b) => b.name.toLowerCase() === folded)
        ?.name ?? null
    );
  };
  const existing = useBranchStore((s) => {
    if (!preview) return null;
    const folded = preview.toLowerCase();
    return s.branches?.local.find((b) => b.name.toLowerCase() === folded)?.name ?? null;
  });

  useAsyncEffect(
    async (live) => {
      const p = await previewBranchName(name);
      if (live()) setPreview(p);
    },
    [name],
  );

  const create = async () => {
    // Recompute rather than use `preview`: Enter can land before the preview for the
    // latest keystroke arrives, which would create the previous name.
    const branch = await previewBranchName(name);
    if (!branch || takenBy(branch) !== null) return;
    const res = await mutate((id) => createBranch(id, branch));
    if (res?.status === "ok") {
      setName("");
      close();
    }
  };

  return (
    <Modal open={open} title="Create a branch" onClose={close}>
      <form
        className="form"
        onSubmit={(e) => {
          e.preventDefault();
          void create();
        }}
      >
        <label>
          Name
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            autoFocus
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
          />
        </label>
        {name.trim() !== "" && preview !== name.trim() && (
          <p className="muted">
            Will be created as <code>{preview || "(no valid characters)"}</code>
          </p>
        )}
        {existing !== null && (
          <p className="form-error" role="alert">
            A branch named <code>{existing}</code> already exists.
            {existing !== preview &&
              " Names that differ only in capital letters get mixed up on Windows and Mac."}
          </p>
        )}
        <p className="muted">
          Starts from {from ? <strong>{from}</strong> : "the current commit"}. Your uncommitted
          changes come with you.
        </p>
        <div className="dialog-actions">
          <button type="button" className="secondary" onClick={close}>
            Cancel
          </button>
          <button type="submit" disabled={!preview || existing !== null}>
            Create branch
          </button>
        </div>
      </form>
    </Modal>
  );
}
