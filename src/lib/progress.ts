import type { Progress } from "../bindings";

/** "Receiving objects 45%" for a progress event; "Starting…" before the first one. */
export function formatProgress(progress: Progress | null): string {
  if (!progress) return "Starting…";
  return progress.percent === null ? progress.phase : `${progress.phase} ${progress.percent}%`;
}
