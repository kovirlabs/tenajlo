import type { LfsLock } from "../../bindings";

/** Who holds a lock, in plain words. */
export function lockLabel(lock: LfsLock): string {
  return lock.ours ? "Locked by you" : `Locked by ${lock.owner || "someone else"}`;
}

/** A small padlock shown on a locked file; the accent color when it's someone else's. */
export function LockBadge({ lock }: { lock: LfsLock }) {
  const label = lockLabel(lock);
  return (
    <span
      className={lock.ours ? "lock-badge ours" : "lock-badge theirs"}
      title={label}
      aria-label={label}
      role="img"
    >
      <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
        <path
          fill="currentColor"
          d="M4 7V5a4 4 0 1 1 8 0v2h.5A1.5 1.5 0 0 1 14 8.5v5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 13.5v-5A1.5 1.5 0 0 1 3.5 7H4Zm1.5 0h5V5a2.5 2.5 0 0 0-5 0v2Z"
        />
      </svg>
      {!lock.ours && <span className="lock-owner">{lock.owner}</span>}
    </span>
  );
}
