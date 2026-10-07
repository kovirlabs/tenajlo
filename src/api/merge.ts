import { commands, type OperationState } from "../bindings";
import type { Result } from "./result";

/** The merge (or rebase…) in progress, if any, and its conflicted files. */
export function getOperationState(repoId: string): Promise<Result<OperationState>> {
  return commands.getOperationState(repoId);
}

export function markResolved(repoId: string, paths: string[]): Promise<Result<null>> {
  return commands.markResolved(repoId, paths);
}

/** Abandons the merge/rebase in progress; the branch goes back to how it was. */
export function abortOperation(repoId: string): Promise<Result<null>> {
  return commands.abortOperation(repoId);
}

/** Opens a repository file in the editor chosen in Settings. */
export function openRepoFile(repoId: string, path: string): Promise<Result<null>> {
  return commands.openRepoFile(repoId, path);
}

/** Shows a repository file in Explorer / Finder. */
export function revealRepoFile(repoId: string, path: string): Promise<Result<null>> {
  return commands.revealRepoFile(repoId, path);
}
