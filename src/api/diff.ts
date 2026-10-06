import { commands, type FileDiff } from "../bindings";
import type { Result } from "./result";

export function getWorkingDiff(repoId: string, path: string): Promise<Result<FileDiff>> {
  return commands.getWorkingDiff(repoId, path);
}
