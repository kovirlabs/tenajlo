import { commands, type Repository, type RepositoryList } from "../bindings";
import type { Result } from "./result";

export function listRepositories(): Promise<RepositoryList> {
  return commands.listRepositories();
}

/** Opens a folder picker. `data` is null if the user cancelled. */
export function addLocalRepository(): Promise<Result<Repository | null>> {
  return commands.addLocalRepository();
}

export function removeRepository(id: string): Promise<Result<null>> {
  return commands.removeRepository(id);
}

export function selectRepository(id: string): Promise<Result<Repository>> {
  return commands.selectRepository(id);
}

/** Where new repositories go by default (Settings, else Documents/Tenajlo). */
export function defaultRepositoryFolder(): Promise<Result<string>> {
  return commands.defaultRepositoryFolder();
}

/** Creates `parent/name` as a new repository on `main`, then adds and selects it. */
export function createRepository(
  parent: string,
  name: string,
  readme: boolean,
): Promise<Result<Repository>> {
  return commands.createRepository(parent, name, readme);
}
