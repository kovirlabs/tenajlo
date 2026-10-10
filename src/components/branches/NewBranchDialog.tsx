import { useEffect, useState } from "react";
import { createBranch, previewBranchName } from "../../api/branches";
import { useChangesStore } from "../../stores/changesStore";
import { useUiStore } from "../../stores/uiStore";
import { Modal } from "../Modal";

/** "New branch…" — created from the current commit; local changes come along. */
export function NewBranchDialog() {
  const open = useUiStore((s) => s.dialog === "newBranch");
  const close = () => useUiStore.getState().openDialog(null);
  const mutate = useChangesStore((s) => s.mutate);
  const from = useChangesStore((s) => s.status?.branch.name);
  const [name, setName] = useState("");
  const [preview, setPreview] = useState("");

  useEffect(() => {
    let live = true;
    void previewBranchName(name).then((p) => {
      if (live) setPreview(p);
    });
    return () => {
      live = false;
    };
  }, [name]);

  const create = async () => {
    // Recompute rather than use `preview`: Enter can land before the preview for the
    // latest keystroke arrives, which would create the previous name.
    const branch = await previewBranchName(name);
    if (!branch) return;
    const ok = await mutate((id) => createBranch(id, branch));
    if (ok) {
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
          <input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
        </label>
        {name.trim() !== "" && preview !== name.trim() && (
          <p className="muted">
            Will be created as <code>{preview || "(no valid characters)"}</code>
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
          <button type="submit" disabled={!preview}>
            Create branch
          </button>
        </div>
      </form>
    </Modal>
  );
}
