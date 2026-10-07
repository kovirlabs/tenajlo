import { commands, type Identity } from "../bindings";
import type { Result } from "./result";

/** Stages (`staged = true`) or unstages changed files. */
export function setStaged(repoId: string, paths: string[], staged: boolean): Promise<Result<null>> {
  return commands.setStaged(repoId, paths, staged);
}

/** Commits staged changes; resolves to the new commit SHA. */
export function commitChanges(
  repoId: string,
  summary: string,
  description: string,
): Promise<Result<string>> {
  return commands.commitChanges(repoId, summary, description);
}

export function getIdentity(repoId: string): Promise<Result<Identity>> {
  return commands.getIdentity(repoId);
}

/** Writes user.name / user.email to the GLOBAL git config. Only after explicit user confirmation. */
export function setGlobalIdentity(name: string, email: string): Promise<Result<null>> {
  return commands.setGlobalIdentity(name, email);
}
