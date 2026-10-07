import { useState } from "react";
import { deleteBranch } from "../../api/branches";
import { useChangesStore } from "../../stores/changesStore";
import { ConfirmDialog } from "../ConfirmDialog";

type Props = { name: string | null; onClose: () => void };

/** Confirms deleting a local branch; asks again before force-deleting unmerged work. */
export function DeleteBranchDialog({ name, onClose }: Props) {
  const mutate = useChangesStore((s) => s.mutate);
  const [unmerged, setUnmerged] = useState(false);

  const close = () => {
    setUnmerged(false);
    onClose();
  };

  const confirm = async () => {
    if (!name) return;
    const force = unmerged;
    let needsForce = false;
    await mutate(async (id) => {
      const res = await deleteBranch(id, name, force);
      if (res.status === "error" && res.error.gitKind === "BranchNotMerged" && !force) {
        needsForce = true;
        return { status: "ok" };
      }
      return res;
    });
    if (needsForce) setUnmerged(true);
    else close();
  };

  return (
    <ConfirmDialog
      open={name !== null}
      title={unmerged ? "Delete unmerged branch?" : "Delete branch?"}
      confirmLabel={unmerged ? "Delete anyway" : "Delete"}
      onCancel={close}
      onConfirm={() => void confirm()}
    >
      {unmerged ? (
        <p>
          <strong>{name}</strong> has commits that aren't on any other branch. If you delete it,
          that work will be hard to get back.
        </p>
      ) : (
        <p>
          Delete the branch <strong>{name}</strong> from this computer? It won't be removed from the
          server.
        </p>
      )}
    </ConfirmDialog>
  );
}
