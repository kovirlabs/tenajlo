import { create } from "zustand";
import type { AppError, LfsLock } from "../bindings";
import { listLfsLocks, lockLfsFile, lockPickedLfsFile, unlockLfsFile } from "../api/locks";
import { useUiStore } from "./uiStore";

type LockState = {
  repoId: string | null;
  /** Locks apply to this repository (Git LFS, a remote, a server with the locking API). */
  available: boolean;
  locks: LfsLock[];
  /** The last listing failed (shown in the File locks dialog only). */
  error: AppError | null;
  loading: boolean;
  /** A lock or unlock is running. */
  busy: boolean;
  /** Reloads the list. `background` (after a fetch) never prompts and keeps old data on error. */
  load: (repoId: string, background: boolean) => Promise<void>;
  lock: (path: string) => Promise<void>;
  lockPicked: () => Promise<void>;
  unlock: (lock: LfsLock, force: boolean) => Promise<void>;
};

const sortByPath = (locks: LfsLock[]) => [...locks].sort((a, b) => a.path.localeCompare(b.path));

export const useLockStore = create<LockState>((set, get) => {
  /** Bumped by every lock or unlock, so a listing that started earlier doesn't undo it. */
  let changes = 0;
  /** Runs a lock change for the current repository; errors go to the error dialog. */
  const change = async <T>(
    run: (
      repoId: string,
    ) => Promise<{ status: "ok"; data: T } | { status: "error"; error: AppError }>,
    apply: (data: T) => void,
  ) => {
    const repoId = get().repoId;
    if (!repoId || get().busy) return;
    set({ busy: true });
    const res = await run(repoId);
    if (get().repoId !== repoId) return;
    set({ busy: false });
    changes += 1;
    if (res.status === "ok") return apply(res.data);
    if (res.error.kind === "GitCancelled") return;
    useUiStore.getState().showError(res.error);
    // The server may not match the list any more (the lock was already removed elsewhere, or
    // someone else took it first): show what's there now, without prompting.
    void get().load(repoId, true);
  };
  const add = (lock: LfsLock | null) => {
    if (lock)
      set({ locks: sortByPath([...get().locks.filter((l) => l.path !== lock.path), lock]) });
  };

  return {
    repoId: null,
    available: false,
    locks: [],
    error: null,
    loading: false,
    busy: false,

    load: async (repoId, background) => {
      if (get().repoId !== repoId) {
        set({ repoId, available: false, locks: [], error: null, busy: false });
      }
      set({ loading: true });
      const startedAt = changes;
      const res = await listLfsLocks(repoId, background);
      if (get().repoId !== repoId) return;
      if (startedAt !== changes) {
        // A lock or unlock finished meanwhile; this list may not include it.
        set({ loading: false });
        return;
      }
      if (res.status === "ok") {
        set({ available: res.data.available, locks: res.data.locks, error: null, loading: false });
      } else {
        set({ error: res.error, loading: false });
      }
    },

    lock: (path) => change((id) => lockLfsFile(id, path), add),
    lockPicked: () => change(lockPickedLfsFile, add),
    unlock: (lock, force) =>
      change(
        (id) => unlockLfsFile(id, lock.id, force),
        () => set({ locks: get().locks.filter((l) => l.id !== lock.id) }),
      ),
  };
});

/** The lock on `path` in the open repository, if any. */
export function useLockFor(path: string): LfsLock | undefined {
  return useLockStore((s) => (s.available ? s.locks.find((l) => l.path === path) : undefined));
}
