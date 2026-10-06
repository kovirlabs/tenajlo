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
