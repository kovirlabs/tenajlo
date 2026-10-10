import { commands, type Commit, type CommitFile, type WorkingDiff } from "../bindings";
import type { Result } from "./result";

export function getHistory(repoId: string, skip: number, limit: number): Promise<Result<Commit[]>> {
  return commands.getHistory(repoId, skip, limit);
}

export function getCommitFiles(repoId: string, sha: string): Promise<Result<CommitFile[]>> {
  return commands.getCommitFiles(repoId, sha);
}

/** A file's diff in a commit. Committed lines can't be staged, so `lines` is always null. */
export async function getCommitDiff(
  repoId: string,
  sha: string,
  path: string,
  oldPath: string | null,
): Promise<Result<WorkingDiff>> {
  const res = await commands.getCommitDiff(repoId, sha, path, oldPath);
  return res.status === "ok" ? { status: "ok", data: { diff: res.data, lines: null } } : res;
}
