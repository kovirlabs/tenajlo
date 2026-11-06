import { commands, type LfsLock, type LfsLocks } from "../bindings";
import type { Result } from "./result";

/** Every Git LFS lock on the server (spec §8.1, v1.4). `background` never prompts. */
export function listLfsLocks(repoId: string, background: boolean): Promise<Result<LfsLocks>> {
  return commands.listLfsLocks(repoId, background);
}

/** Locks a repository-relative path. */
export function lockLfsFile(repoId: string, path: string): Promise<Result<LfsLock>> {
  return commands.lockLfsFile(repoId, path);
}

/** Shows a file picker limited to the repository and locks the choice (`null`: cancelled). */
export function lockPickedLfsFile(repoId: string): Promise<Result<LfsLock | null>> {
  return commands.lockPickedLfsFile(repoId);
}

/** Removes a lock by id; `force` removes someone else's. */
export function unlockLfsFile(
  repoId: string,
  lockId: string,
  force: boolean,
): Promise<Result<null>> {
  return commands.unlockLfsFile(repoId, lockId, force);
}
