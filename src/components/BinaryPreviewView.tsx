import { useState } from "react";
import type { AppError, BinaryPreview, FileVersion } from "../bindings";
import type { Result } from "../api/result";
import { useAsyncEffect } from "../hooks/useAsyncEffect";
import { formatSize, formatSizeChange } from "../lib/size";
import { useLockFor } from "../stores/lockStore";
import { lockLabel } from "./locks/LockBadge";

/** How to load the preview for the file shown. */
export type PreviewProps = {
  key: string;
  /** Repository-relative path, for the lock state. */
  path: string;
  fetch: () => Promise<Result<BinaryPreview>>;
};

/**
 * Shown instead of "binary file changed" (spec §8.1, v1.4): sizes before and after, images
 * side by side, and for Git LFS files a note and the lock state.
 */
export function BinaryPreviewView({ preview, lfs }: { preview: PreviewProps; lfs: boolean }) {
  const [data, setData] = useState<{ key: string; preview: BinaryPreview } | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const lock = useLockFor(preview.path);

  useAsyncEffect(
    async (live) => {
      setError(null);
      const res = await preview.fetch();
      if (!live()) return;
      if (res.status === "ok") setData({ key: preview.key, preview: res.data });
      else setError(res.error);
    },
    [preview.key],
  );

  const shown = data?.key === preview.key ? data.preview : null;
  const isLfs = lfs || shown?.old?.lfs === true || shown?.new?.lfs === true;
  return (
    <div className="binary-preview">
      <p className="binary-summary">
        {isLfs ? <span className="lfs-tag">Stored with Git LFS</span> : <span>Binary file</span>}
        {lock && <span className="muted"> · {lockLabel(lock)}</span>}
      </p>
      {error && <p className="muted">{error.message}</p>}
      {!shown && !error && <p className="muted">Loading…</p>}
      {shown && <Sizes preview={shown} />}
      {shown && (shown.old?.image || shown.new?.image) && (
        <div className="image-compare">
          <Side label="Before" version={shown.old} />
          <Side label="After" version={shown.new} />
        </div>
      )}
    </div>
  );
}

function Sizes({ preview }: { preview: BinaryPreview }) {
  const { old, new: next } = preview;
  if (!old && next) return <p>New file · {formatSize(next.size)}</p>;
  if (old && !next) return <p>Deleted · was {formatSize(old.size)}</p>;
  if (!old || !next) return null;
  const change = formatSizeChange(old.size, next.size);
  return (
    <p>
      {formatSize(old.size)} → {formatSize(next.size)}
      {change ? (
        <span className="muted"> ({change})</span>
      ) : (
        <span className="muted"> (same size)</span>
      )}
    </p>
  );
}

function Side({ label, version }: { label: string; version: FileVersion | null }) {
  return (
    <figure className="image-side">
      <figcaption className="muted">{label}</figcaption>
      {version?.image ? (
        <img src={version.image} alt={`${label} version`} />
      ) : (
        <div className="image-missing muted">
          {version === null
            ? "No file"
            : version.lfs
              ? "Not downloaded to this computer"
              : "No preview"}
        </div>
      )}
    </figure>
  );
}
