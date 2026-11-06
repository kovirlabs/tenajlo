import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppError, LfsLock } from "../bindings";
import { listLfsLocks, lockLfsFile, unlockLfsFile } from "../api/locks";
import { useLockStore } from "./lockStore";
import { useUiStore } from "./uiStore";

vi.mock("../api/locks", () => ({
  listLfsLocks: vi.fn(),
  lockLfsFile: vi.fn(),
  lockPickedLfsFile: vi.fn(),
  unlockLfsFile: vi.fn(),
}));

const lock = (path: string, ours: boolean, id = path): LfsLock => ({
  id,
  path,
  owner: ours ? "me" : "alice",
  lockedAt: "2026-11-05T23:54:48Z",
  ours,
});

const failure: AppError = {
  kind: "Git",
  gitKind: "LfsLockNotYours",
  message: "This lock belongs to someone else.",
  details: null,
  accountId: null,
};

describe("lockStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useLockStore.setState({ repoId: null, available: false, locks: [], error: null, busy: false });
    useUiStore.setState({ error: null });
  });

  it("loads locks and resets when the repository changes", async () => {
    vi.mocked(listLfsLocks).mockResolvedValueOnce({
      status: "ok",
      data: { available: true, locks: [lock("a.bin", true)] },
    });
    await useLockStore.getState().load("r1", true);
    expect(useLockStore.getState()).toMatchObject({ available: true, repoId: "r1" });
    expect(listLfsLocks).toHaveBeenCalledWith("r1", true);

    vi.mocked(listLfsLocks).mockResolvedValueOnce({ status: "error", error: failure });
    await useLockStore.getState().load("r2", true);
    expect(useLockStore.getState()).toMatchObject({ available: false, locks: [], error: failure });
  });

  it("keeps the last list when a background refresh fails", async () => {
    useLockStore.setState({ repoId: "r1", available: true, locks: [lock("a.bin", true)] });
    vi.mocked(listLfsLocks).mockResolvedValueOnce({ status: "error", error: failure });
    await useLockStore.getState().load("r1", true);
    expect(useLockStore.getState().locks).toHaveLength(1);
  });

  it("adds a new lock and removes an unlocked one without reloading", async () => {
    useLockStore.setState({ repoId: "r1", available: true, locks: [lock("b.bin", false)] });
    vi.mocked(lockLfsFile).mockResolvedValueOnce({ status: "ok", data: lock("a.bin", true, "9") });
    await useLockStore.getState().lock("a.bin");
    expect(useLockStore.getState().locks.map((l) => l.path)).toEqual(["a.bin", "b.bin"]);

    vi.mocked(unlockLfsFile).mockResolvedValueOnce({ status: "ok", data: null });
    await useLockStore.getState().unlock(lock("a.bin", true, "9"), false);
    expect(unlockLfsFile).toHaveBeenCalledWith("r1", "9", false);
    expect(useLockStore.getState().locks.map((l) => l.path)).toEqual(["b.bin"]);
    expect(listLfsLocks).not.toHaveBeenCalled();
  });

  it("shows a failed unlock in the error dialog", async () => {
    useLockStore.setState({ repoId: "r1", available: true, locks: [lock("b.bin", false)] });
    vi.mocked(unlockLfsFile).mockResolvedValueOnce({ status: "error", error: failure });
    // Meanwhile the lock was removed elsewhere: the quiet reload after the error shows that.
    vi.mocked(listLfsLocks).mockResolvedValueOnce({
      status: "ok",
      data: { available: true, locks: [] },
    });
    await useLockStore.getState().unlock(lock("b.bin", false), false);
    expect(useUiStore.getState().error).toEqual(failure);
    expect(useLockStore.getState()).toMatchObject({ busy: false });
    expect(listLfsLocks).toHaveBeenCalledWith("r1", true);
    await vi.waitFor(() => expect(useLockStore.getState().locks).toHaveLength(0));
  });

  it("ignores a listing that started before a lock finished", async () => {
    useLockStore.setState({ repoId: "r1", available: true, locks: [] });
    let finishList: (v: Awaited<ReturnType<typeof listLfsLocks>>) => void = () => {};
    vi.mocked(listLfsLocks).mockReturnValueOnce(new Promise((r) => (finishList = r)));
    const loading = useLockStore.getState().load("r1", true);
    vi.mocked(lockLfsFile).mockResolvedValueOnce({ status: "ok", data: lock("a.bin", true) });
    await useLockStore.getState().lock("a.bin");
    finishList({ status: "ok", data: { available: true, locks: [] } });
    await loading;
    expect(useLockStore.getState().locks.map((l) => l.path)).toEqual(["a.bin"]);
  });
});
