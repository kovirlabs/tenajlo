import { commands, type AppInfo, type GitInfo } from "../bindings";
import type { Result } from "./result";

/** Locates git and checks it meets Tenajlo's minimum version. */
export function checkGit(): Promise<Result<GitInfo>> {
  return commands.checkGit();
}

/** Tenajlo's version and log folder (Settings → About). */
export function getAppInfo(): Promise<Result<AppInfo>> {
  return commands.getAppInfo();
}

export function openLogsFolder(): Promise<Result<null>> {
  return commands.openLogsFolder();
}
