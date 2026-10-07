import { commands, type LfsStatus, type WorkingDirectoryStatus } from "../bindings";
import type { Result } from "./result";

export function getStatus(repoId: string): Promise<Result<WorkingDirectoryStatus>> {
  return commands.getStatus(repoId);
}

/** Whether the repository uses Git LFS and whether git-lfs is installed. */
export function getLfsStatus(repoId: string): Promise<Result<LfsStatus>> {
  return commands.getLfsStatus(repoId);
}
