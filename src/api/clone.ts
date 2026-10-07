import { commands, type RemoteRepository, type Repository } from "../bindings";
import type { Result } from "./result";

/** The account's repositories and its organizations' repositories. */
export function listForgejoRepositories(accountId: string): Promise<Result<RemoteRepository[]>> {
  return commands.listForgejoRepositories(accountId);
}

/** Default destination for a clone of `url` (Documents/Tenajlo/<name>). */
export function suggestClonePath(url: string): Promise<Result<string>> {
  return commands.suggestClonePath(url);
}

/** Folder picker; `data` is `<picked>/<name>`, or null if cancelled. */
export function chooseCloneFolder(url: string): Promise<Result<string | null>> {
  return commands.chooseCloneFolder(url);
}

/** Clones and adds the repository. Progress events carry `opId` and an empty `repoId`. */
export function cloneRepository(
  opId: string,
  url: string,
  path: string,
): Promise<Result<Repository>> {
  return commands.cloneRepository(opId, url, path);
}
