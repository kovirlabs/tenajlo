import { useEffect } from "react";
import type { LineRef, WorkingDiff } from "../bindings";
import type { Result } from "../api/result";
import { DiffView } from "../components/DiffView";
import { ErrorDetails } from "../components/ErrorDetails";
import { useDiffStore } from "../stores/diffStore";

type Props = {
  /** What to show; null shows `empty`. Changing it (or `version`) reloads. */
  source: { key: string; title: string; fetch: () => Promise<Result<WorkingDiff>> } | null;
  /** Bump to re-fetch the same file (e.g. after the working tree changed). */
  version?: unknown;
  empty: string;
  /** Stages or unstages lines, for diffs that support it. `token` identifies the diff shown. */
  onStageLines?: ((token: string, lines: LineRef[], staged: boolean) => void) | undefined;
  /** Disables line staging while another operation runs. */
  busy?: boolean;
};

export function DiffPane({ source, version, empty, onStageLines, busy = false }: Props) {
  const key = useDiffStore((s) => s.key);
  const diff = useDiffStore((s) => s.diff);
  const lines = useDiffStore((s) => s.lines);
  const error = useDiffStore((s) => s.error);
  const load = useDiffStore((s) => s.load);
  const clear = useDiffStore((s) => s.clear);

  const sourceKey = source?.key ?? null;
  useEffect(() => {
    if (!source) clear();
    else void load(source.key, source.fetch);
    // `source.fetch` is recreated each render; reload only when the key or version changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sourceKey, version]);

  if (!source) return <p className="pane-message muted">{empty}</p>;
  return (
    <>
      <div className="detail-header" title={source.title}>
        {source.title}
      </div>
      {error && key === source.key ? (
        <div className="pane-message">
          <p>{error.message}</p>
          <ErrorDetails details={error.details} />
        </div>
      ) : diff && key === source.key ? (
        <DiffView
          diff={diff}
          staging={
            lines && onStageLines
              ? {
                  lines,
                  disabled: busy,
                  onChange: (refs, staged) => onStageLines(lines.token, refs, staged),
                }
              : undefined
          }
        />
      ) : (
        <p className="pane-message muted">Loading…</p>
      )}
    </>
  );
}
