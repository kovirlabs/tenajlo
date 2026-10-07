import { restoreSavedChanges } from "../api/branches";
import { useChangesStore } from "../stores/changesStore";

/** Shown when Tenajlo saved changes on this branch during an earlier switch. */
export function SavedChangesBanner() {
  const saved = useChangesStore((s) => s.saved);
  const busy = useChangesStore((s) => s.busy);
  const mutate = useChangesStore((s) => s.mutate);
  if (!saved) return null;
  return (
    <div className="banner" role="status">
      <span>You have changes saved from when you last left this branch.</span>
      <button type="button" disabled={busy} onClick={() => void mutate(restoreSavedChanges)}>
        Restore
      </button>
    </div>
  );
}
