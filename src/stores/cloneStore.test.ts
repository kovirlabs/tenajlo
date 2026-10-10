import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RemoteRepository } from "../bindings";
import { cloneRepository, listForgejoRepositories } from "../api/clone";
import { cancelOperation } from "../api/sync";
import { useCloneStore } from "./cloneStore";

vi.mock("../api/clone", () => ({
  listForgejoRepositories: vi.fn(),
  cloneRepository: vi.fn(),
}));
vi.mock("../api/sync", () => ({ cancelOperation: vi.fn(async () => {}) }));

const mockList = vi.mocked(listForgejoRepositories);
const mockClone = vi.mocked(cloneRepository);
const repo = { fullName: "team/plc" } as RemoteRepository;
const progress = { phase: "Receiving objects", percent: 50, detail: null, remote: false };

describe("cloneStore", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useCloneStore.setState({ lists: {}, loadingAccount: null, listError: null, running: null });
  });

  it("reuses a recent repository list unless forced", async () => {
    mockList.mockResolvedValue({ status: "ok", data: [repo] });
    await useCloneStore.getState().loadRepos("acct");
    await useCloneStore.getState().loadRepos("acct");
    expect(mockList).toHaveBeenCalledTimes(1);
    expect(useCloneStore.getState().lists.acct?.repos).toEqual([repo]);

    await useCloneStore.getState().loadRepos("acct", true);
    expect(mockList).toHaveBeenCalledTimes(2);
  });

  it("tracks the running clone's progress and cancels it", async () => {
    let finish: () => void = () => {};
    mockClone.mockReturnValue(
      new Promise((r) => (finish = () => r({ status: "ok", data: {} as never }))),
    );
    const done = useCloneStore.getState().clone("https://h/team/plc.git", "/tmp/plc");
    const opId = useCloneStore.getState().running?.opId ?? "";
    expect(opId).not.toBe("");

    useCloneStore.getState().onProgress("someone-else", progress);
    expect(useCloneStore.getState().running?.progress).toBeNull();
    useCloneStore.getState().onProgress(opId, progress);
    expect(useCloneStore.getState().running?.progress).toEqual(progress);

    useCloneStore.getState().cancel();
    expect(cancelOperation).toHaveBeenCalledWith(opId);

    finish();
    await done;
    expect(useCloneStore.getState().running).toBeNull();
  });
});
