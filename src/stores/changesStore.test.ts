import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppError, FileChange, WorkingDirectoryStatus } from "../bindings";
import { getStatus } from "../api/status";
import { useChangesStore } from "./changesStore";
import { registerRefresh } from "./refreshRepo";
import { useUiStore } from "./uiStore";

vi.mock("../api/status", () => ({ getStatus: vi.fn() }));
vi.mock("../api/branches", () => ({
  getSavedChanges: vi.fn(async () => ({ status: "ok", data: null })),
}));
vi.mock("../api/changes", () => ({ setStaged: vi.fn() }));

const mockGetStatus = vi.mocked(getStatus);

const file = (path: string): FileChange => ({
  path,
  oldPath: null,
  kind: "Modified",
  staged: "None",
  submodule: false,
});

const status = (...paths: string[]): WorkingDirectoryStatus => ({
  branch: { name: "main", tip: "abc", upstream: null, ahead: 0, behind: 0, upstreamGone: false },
  files: paths.map(file),
  hasConflicts: false,
});

const failure: AppError = {
  kind: "Git",
  gitKind: "BranchNotMerged",
  message: "boom",
  details: null,
  accountId: null,
};

describe("changesStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // `mutate` refreshes this store too.
    mockGetStatus.mockResolvedValue({ status: "ok", data: status() });
    useChangesStore.getState().reset();
    useUiStore.setState({ error: null });
  });

  it("keeps the selected file across refreshes while it still has changes", async () => {
    mockGetStatus.mockResolvedValueOnce({ status: "ok", data: status("a.txt", "b.txt") });
    await useChangesStore.getState().refresh("r1");
    expect(useChangesStore.getState().selectedPath).toBe("a.txt");

    useChangesStore.getState().selectFile("b.txt");
    mockGetStatus.mockResolvedValueOnce({ status: "ok", data: status("a.txt", "b.txt") });
    await useChangesStore.getState().refresh("r1");
    expect(useChangesStore.getState().selectedPath).toBe("b.txt");

    mockGetStatus.mockResolvedValueOnce({ status: "ok", data: status("a.txt") });
    await useChangesStore.getState().refresh("r1");
    expect(useChangesStore.getState().selectedPath).toBe("a.txt");
  });

  it("ignores a status that arrives after switching repositories", async () => {
    let resolveOld: (v: Awaited<ReturnType<typeof getStatus>>) => void = () => {};
    mockGetStatus.mockReturnValueOnce(new Promise((r) => (resolveOld = r)));
    const old = useChangesStore.getState().refresh("old");
    mockGetStatus.mockResolvedValueOnce({ status: "ok", data: status("new.txt") });
    await useChangesStore.getState().refresh("new");
    resolveOld({ status: "ok", data: status("old.txt") });
    await old;
    expect(useChangesStore.getState().status?.files[0]?.path).toBe("new.txt");
  });

  it("mutate returns the result and refreshes every registered store once", async () => {
    const refreshed = vi.fn(async () => {});
    registerRefresh("test", refreshed);
    useChangesStore.setState({ repoId: "r1" });

    const res = await useChangesStore
      .getState()
      .mutate(async () => ({ status: "ok" as const, data: "sha1" }));
    expect(res).toEqual({ status: "ok", data: "sha1" });
    expect(refreshed).toHaveBeenCalledTimes(1);
    expect(refreshed).toHaveBeenCalledWith("r1");
    expect(useChangesStore.getState().busy).toBe(false);
  });

  it("mutate reports errors unless the caller handles them", async () => {
    useChangesStore.setState({ repoId: "r1" });
    const fail = async () => ({ status: "error" as const, error: failure });

    await useChangesStore
      .getState()
      .mutate(fail, { quietError: (e) => e.gitKind === "BranchNotMerged" });
    expect(useUiStore.getState().error).toBeNull();

    await useChangesStore.getState().mutate(fail);
    expect(useUiStore.getState().error).toBe(failure);
  });

  it("mutate doesn't run while another operation is running", async () => {
    useChangesStore.setState({ repoId: "r1", busy: true });
    const op = vi.fn();
    expect(await useChangesStore.getState().mutate(op)).toBeNull();
    expect(op).not.toHaveBeenCalled();
  });
});
