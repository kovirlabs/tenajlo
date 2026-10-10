import { commands, events, type AvailableUpdate, type UpdateProgress } from "../bindings";
import type { Result } from "./result";

/** Asks GitHub Releases whether a newer Tenajlo exists; `null` means this is the newest. */
export function checkForUpdate(): Promise<Result<AvailableUpdate | null>> {
  return commands.checkForUpdate();
}

/** Downloads, verifies and installs the update, then restarts Tenajlo. */
export function installUpdate(): Promise<Result<null>> {
  return commands.installUpdate();
}

/** Opens the latest release page in the browser. */
export function openReleasePage(): Promise<Result<null>> {
  return commands.openReleasePage();
}

export function onUpdateProgress(cb: (p: UpdateProgress) => void): Promise<() => void> {
  return events.updateProgress.listen((e) => cb(e.payload));
}
