import { commands, events } from "../bindings";
import type { Result } from "./result";

/** Starts watching a repository; replaces any previous watch. */
export function watchRepository(repoId: string): Promise<Result<null>> {
  return commands.watchRepository(repoId);
}

/** Subscribes to repository change notifications. Resolves to an unsubscribe function. */
export function onRepoChanged(cb: (repoId: string) => void): Promise<() => void> {
  return events.repoChanged.listen((e) => cb(e.payload.repoId));
}
