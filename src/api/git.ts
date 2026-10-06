import { commands, type GitInfo } from "../bindings";
import type { Result } from "./result";

/** Locates git and checks it meets Anvil's minimum version. */
export function checkGit(): Promise<Result<GitInfo>> {
  return commands.checkGit();
}
