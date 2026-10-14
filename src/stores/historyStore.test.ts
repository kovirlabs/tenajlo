import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppError, Commit } from "../bindings";
import { getHistory } from "../api/history";
import { PAGE_SIZE, useHistoryStore } from "./historyStore";

vi.mock("../api/history", () => ({
  getHistory: vi.fn(),
  getCommitFiles: vi.fn(async () => ({ status: "ok", data: [] })),
  getCommitDiff: vi.fn(),
}));

const mockGetHistory = vi.mocked(getHistory);

const commit = (n: number): Commit => ({
  sha: `sha${n}`,
  shortSha: `sha${n}`,
  parents: [],
  authorName: "Evan",
  authorEmail: "evan@example.com",
  authorDate: "2026-01-01T00:00:00+00:00",
  summary: `commit ${n}`,
  body: "",
  unpushed: false,
});

const page = (from: number, count: number) =>
  Array.from({ length: count }, (_, i) => commit(from + i));

const failure: AppError = {
  kind: "Internal",
  gitKind: null,
  message: "boom",
  details: null,
  accountId: null,
};

describe("historyStore.refresh selection", () => {
  const history = (...ns: number[]) => ({ status: "ok" as const, data: ns.map(commit) });

  beforeEach(async () => {
    vi.clearAllMocks();
    useHistoryStore.setState({ repoId: null });
    mockGetHistory.mockResolvedValueOnce(history(1));
    await useHistoryStore.getState().refresh("r1");
  });

  it("moves to the newest commit when the user hasn't picked one", async () => {
    expect(useHistoryStore.getState().selectedSha).toBe("sha1");
    mockGetHistory.mockResolvedValueOnce(history(2, 1));
    await useHistoryStore.getState().refresh("r1");
    expect(useHistoryStore.getState().selectedSha).toBe("sha2");
  });

  it("keeps an older commit the user picked", async () => {
    mockGetHistory.mockResolvedValueOnce(history(2, 1));
    await useHistoryStore.getState().refresh("r1");
    await useHistoryStore.getState().selectCommit("sha1");
    mockGetHistory.mockResolvedValueOnce(history(3, 2, 1));
    await useHistoryStore.getState().refresh("r1");
    expect(useHistoryStore.getState().selectedSha).toBe("sha1");
  });

  it("goes back to following the newest when the picked commit is gone", async () => {
    mockGetHistory.mockResolvedValueOnce(history(2, 1));
    await useHistoryStore.getState().refresh("r1");
    await useHistoryStore.getState().selectCommit("sha1");
    mockGetHistory.mockResolvedValueOnce(history(4, 3));
    await useHistoryStore.getState().refresh("r1");
    expect(useHistoryStore.getState().selectedSha).toBe("sha4");
    mockGetHistory.mockResolvedValueOnce(history(5, 4, 3));
    await useHistoryStore.getState().refresh("r1");
    expect(useHistoryStore.getState().selectedSha).toBe("sha5");
  });
});

describe("historyStore.loadMore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useHistoryStore.setState({
      repoId: "r1",
      commits: page(0, PAGE_SIZE),
      complete: false,
      loading: false,
      error: null,
    });
  });

  it("appends the next page", async () => {
    mockGetHistory.mockResolvedValueOnce({ status: "ok", data: page(PAGE_SIZE, 3) });
    await useHistoryStore.getState().loadMore();
    expect(mockGetHistory).toHaveBeenCalledWith("r1", PAGE_SIZE, PAGE_SIZE);
    expect(useHistoryStore.getState().commits).toHaveLength(PAGE_SIZE + 3);
    expect(useHistoryStore.getState().complete).toBe(true);
  });

  it("does not retry after a failure until the next refresh", async () => {
    mockGetHistory.mockResolvedValueOnce({ status: "error", error: failure });
    await useHistoryStore.getState().loadMore();
    await useHistoryStore.getState().loadMore();
    expect(mockGetHistory).toHaveBeenCalledTimes(1);
    expect(useHistoryStore.getState().error).toBe(failure);
  });

  it("drops a page that no longer lines up after a refresh", async () => {
    let resolve: (v: Awaited<ReturnType<typeof getHistory>>) => void = () => {};
    mockGetHistory.mockReturnValueOnce(new Promise((r) => (resolve = r)));
    const pending = useHistoryStore.getState().loadMore();
    useHistoryStore.setState({ commits: page(100, 5), loading: false });
    resolve({ status: "ok", data: page(PAGE_SIZE, 3) });
    await pending;
    expect(useHistoryStore.getState().commits.map((c) => c.sha)).toEqual(
      page(100, 5).map((c) => c.sha),
    );
  });
});
