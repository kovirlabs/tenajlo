import { useRepoStore } from "../stores/repoStore";
import { useUiStore } from "../stores/uiStore";

export function NoRepository() {
  const addLocal = useRepoStore((s) => s.addLocal);
  return (
    <main className="centered">
      <div className="empty">
        <h1>No repository selected</h1>
        <p>Clone a repository from the server, or add a folder that's already a Git repository.</p>
        <div className="empty-actions">
          <button type="button" onClick={() => useUiStore.getState().openDialog("clone")}>
            Clone a repository…
          </button>
          <button type="button" className="secondary" onClick={() => void addLocal()}>
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
