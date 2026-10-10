import { commands, type LineRef, type WorkingDiff } from "../bindings";
import type { Result } from "./result";

/** A changed file's diff against the last commit, with which lines are staged. */
export function getWorkingDiff(repoId: string, path: string): Promise<Result<WorkingDiff>> {
  return commands.getWorkingDiff(repoId, path);
}

/**
 * Stages (`staged = true`) or unstages single changed lines of a file. `token` comes from the
 * `getWorkingDiff` result the lines were picked from.
 */
export function setLinesStaged(
  repoId: string,
  path: string,
  token: string,
  lines: LineRef[],
  staged: boolean,
): Promise<Result<null>> {
  return commands.setLinesStaged(repoId, path, token, lines, staged);
}
