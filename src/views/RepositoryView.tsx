import type { Repository } from "../bindings";

/** Main view for an open repository. Changes and History arrive in later M1 steps. */
export function RepositoryView({ repo }: { repo: Repository }) {
  return (
    <main className="centered">
      <p className="muted">{repo.path}</p>
    </main>
  );
}
