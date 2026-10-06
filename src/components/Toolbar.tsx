import { useSelectedRepository } from "../stores/repoStore";
import { BranchDropdown } from "./BranchDropdown";
import { RepoDropdown } from "./RepoDropdown";

export function Toolbar() {
  const repo = useSelectedRepository();
  return (
    <header className="toolbar">
      <RepoDropdown />
      {repo && !repo.missing && <BranchDropdown />}
    </header>
  );
}
