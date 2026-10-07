import { commands, type Identity, type UndoneCommit } from "../bindings";
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

/** Discards all changes to files; current content is moved to the OS trash first. */
export function discardChanges(repoId: string, paths: string[]): Promise<Result<null>> {
  return commands.discardChanges(repoId, paths);
}

/** Adds an untracked file, or all files with its extension, to .gitignore. */
export function ignoreFile(
  repoId: string,
  path: string,
  byExtension: boolean,
): Promise<Result<null>> {
  return commands.ignoreFile(repoId, path, byExtension);
}

/** Undoes the latest commit if it is still `sha`; returns its message. */
export function undoCommit(repoId: string, sha: string): Promise<Result<UndoneCommit>> {
  return commands.undoCommit(repoId, sha);
}
