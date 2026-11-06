import { commands, type BranchLink, type CommitChecks, type ForgejoPage } from "../bindings";
import type { Result } from "./result";

/** The branch on Forgejo and its open pull request; `null` if the repository isn't linked. */
export function getBranchLink(repoId: string, branch: string): Promise<Result<BranchLink | null>> {
  return commands.getBranchLink(repoId, branch);
}

/** A commit's checks on Forgejo; `null` if the repository isn't linked. */
export function getCommitChecks(repoId: string, sha: string): Promise<Result<CommitChecks | null>> {
  return commands.getCommitChecks(repoId, sha);
}

/** Opens a page of the repository's website (Rust builds the URL). */
export function openForgejoPage(repoId: string, page: ForgejoPage): Promise<Result<null>> {
  return commands.openForgejoPage(repoId, page);
}
