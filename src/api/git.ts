import { commands, type GitInfo } from "../bindings";
import type { Result } from "./result";

/** Locates git and checks it meets Tenajlo's minimum version. */
export function checkGit(): Promise<Result<GitInfo>> {
  return commands.checkGit();
}
