import { commands, type BranchList, type LocalChanges, type SavedChanges } from "../bindings";
import type { Result } from "./result";

export function getBranches(repoId: string): Promise<Result<BranchList>> {
  return commands.getBranches(repoId);
}

export function previewBranchName(name: string): Promise<string> {
  return commands.previewBranchName(name);
}

export function createBranch(repoId: string, name: string): Promise<Result<null>> {
  return commands.createBranch(repoId, name);
}

export function switchBranch(
  repoId: string,
  name: string,
  localChanges: LocalChanges,
): Promise<Result<null>> {
  return commands.switchBranch(repoId, name, localChanges);
}

export function deleteBranch(repoId: string, name: string, force: boolean): Promise<Result<null>> {
  return commands.deleteBranch(repoId, name, force);
}

export function getSavedChanges(repoId: string): Promise<Result<SavedChanges | null>> {
  return commands.getSavedChanges(repoId);
}

export function restoreSavedChanges(repoId: string): Promise<Result<null>> {
  return commands.restoreSavedChanges(repoId);
}
