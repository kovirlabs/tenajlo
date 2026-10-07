import { useRepoStore } from "../stores/repoStore";
import { useUiStore } from "../stores/uiStore";

export function NoRepository() {
  const addLocal = useRepoStore((s) => s.addLocal);
  return (
    <main className="centered">
      <div className="empty">
        <h1>No repository selected</h1>
        <p>Add a folder on this computer that's already a Git repository.</p>
        <div className="empty-actions">
          <button type="button" onClick={() => void addLocal()}>
            Add local repository…
          </button>
          <button
            type="button"
            className="secondary"
            onClick={() => useUiStore.getState().openDialog("accounts")}
          >
            Sign in to Forgejo…
          </button>
        </div>
      </div>
    </main>
  );
}
