import type { Repository } from "../bindings";
import { useRepoStore } from "../stores/repoStore";

export function MissingRepository({ repo }: { repo: Repository }) {
  const remove = useRepoStore((s) => s.remove);
  return (
    <main className="centered">
      <div className="empty">
        <h1>Can't find “{repo.name}”</h1>
        <p>
          It was at <code>{repo.path}</code>, but that folder no longer exists. It may have been
          moved or deleted.
        </p>
        <button type="button" onClick={() => void remove(repo.id)}>
          Remove from list
        </button>
      </div>
    </main>
  );
}
