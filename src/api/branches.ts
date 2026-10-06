import { commands, type BranchList } from "../bindings";
import type { Result } from "./result";

export function getBranches(repoId: string): Promise<Result<BranchList>> {
  return commands.getBranches(repoId);
}
