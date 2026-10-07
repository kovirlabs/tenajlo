import { useEffect } from "react";
import { ErrorDialog } from "../components/ErrorDialog";
import { NewBranchDialog } from "../components/branches/NewBranchDialog";
import { useShortcuts } from "../hooks/useShortcuts";
import { Toolbar } from "../components/Toolbar";
import { useRepoStore, useSelectedRepository } from "../stores/repoStore";
import { MissingRepository } from "./MissingRepository";
import { NoRepository } from "./NoRepository";
import { RepositoryView } from "./RepositoryView";

export function AppShell() {
  const loaded = useRepoStore((s) => s.loaded);
  const load = useRepoStore((s) => s.load);
  const repo = useSelectedRepository();

  useEffect(() => {
    void load();
  }, [load]);

  useShortcuts(repo !== null && !repo.missing);

  let body;
  if (!loaded) body = <main className="centered" aria-busy="true" />;
  else if (!repo) body = <NoRepository />;
  else if (repo.missing) body = <MissingRepository repo={repo} />;
  else body = <RepositoryView key={repo.id} repo={repo} />;

  return (
    <div className="shell">
      <Toolbar />
      {body}
      {repo && !repo.missing && <NewBranchDialog key={repo.id} />}
      <ErrorDialog />
    </div>
  );
}
