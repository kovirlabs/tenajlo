import { commands } from "../bindings";
import type { Result } from "./result";

/** Stages (`staged = true`) or unstages changed files. */
export function setStaged(repoId: string, paths: string[], staged: boolean): Promise<Result<null>> {
  return commands.setStaged(repoId, paths, staged);
}
