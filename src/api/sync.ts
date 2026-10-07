import {
  commands,
  events,
  type AuthAnswer,
  type AuthPromptRequested,
  type GitProgress,
  type SyncRequest,
  type SyncState,
} from "../bindings";
import type { Result } from "./result";

export function getSyncState(repoId: string): Promise<Result<SyncState>> {
  return commands.getSyncState(repoId);
}

/** Runs fetch/pull/push/publish; `opId` identifies progress events and cancellation. */
export function sync(repoId: string, opId: string, request: SyncRequest): Promise<Result<null>> {
  return commands.sync(repoId, opId, request, false);
}

/** Periodic fetch: never shows sign-in prompts, fails quietly instead. */
export function backgroundFetch(repoId: string): Promise<Result<null>> {
  return commands.sync(repoId, crypto.randomUUID(), "Fetch", true);
}

export function cancelOperation(opId: string): Promise<void> {
  return commands.cancelOperation(opId);
}

/** Sends the user's answer (or null to cancel) for a sign-in prompt. Secrets go UI → Rust only. */
export function answerAuthPrompt(promptId: string, answer: AuthAnswer | null): Promise<void> {
  return commands.answerAuthPrompt(promptId, answer);
}

export function onProgress(cb: (p: GitProgress) => void): Promise<() => void> {
  return events.gitProgress.listen((e) => cb(e.payload));
}

export function onAuthPrompt(cb: (p: AuthPromptRequested) => void): Promise<() => void> {
  return events.authPromptRequested.listen((e) => cb(e.payload));
}
