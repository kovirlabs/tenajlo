import { commands, type Commit, type CommitFile, type FileDiff } from "../bindings";
import type { Result } from "./result";

export function getHistory(repoId: string, skip: number, limit: number): Promise<Result<Commit[]>> {
  return commands.getHistory(repoId, skip, limit);
}

export function getCommitFiles(repoId: string, sha: string): Promise<Result<CommitFile[]>> {
  return commands.getCommitFiles(repoId, sha);
}

export function getCommitDiff(
  repoId: string,
  sha: string,
  path: string,
  oldPath: string | null,
): Promise<Result<FileDiff>> {
  return commands.getCommitDiff(repoId, sha, path, oldPath);
}
