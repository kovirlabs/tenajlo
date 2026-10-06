import { commands, type WorkingDirectoryStatus } from "../bindings";
import type { Result } from "./result";

export function getStatus(repoId: string): Promise<Result<WorkingDirectoryStatus>> {
  return commands.getStatus(repoId);
}
