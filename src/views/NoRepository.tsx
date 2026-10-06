import { useRepoStore } from "../stores/repoStore";

export function NoRepository() {
  const addLocal = useRepoStore((s) => s.addLocal);
  return (
    <main className="centered">
      <div className="empty">
        <h1>No repository selected</h1>
        <p>Add a folder on this computer that's already a Git repository.</p>
        <button type="button" onClick={() => void addLocal()}>
          Add local repository…
        </button>
      </div>
    </main>
  );
}
