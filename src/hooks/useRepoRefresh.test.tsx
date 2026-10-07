import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, waitFor } from "@testing-library/react";
import { useBranchStore } from "../stores/branchStore";
import { useChangesStore } from "../stores/changesStore";
import { useHistoryStore } from "../stores/historyStore";
import { useSyncStore } from "../stores/syncStore";
import { useRepoRefresh } from "./useRepoRefresh";

let emit: ((repoId: string) => void) | null = null;
const unlisten = vi.fn();
vi.mock("../api/watch", () => ({
  watchRepository: vi.fn(async () => ({ status: "ok", data: null })),
  onRepoChanged: vi.fn(async (cb: (id: string) => void) => {
    emit = cb;
    return unlisten;
  }),
}));

vi.mock("../api/sync", () => ({
  onProgress: vi.fn(async () => unlisten),
}));

function Probe({ id }: { id: string }) {
  useRepoRefresh(id);
  return null;
}

describe("useRepoRefresh", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("refreshes on open, on matching repo-changed events, and on focus", async () => {
    const changes = vi.fn(async () => {});
    const branches = vi.fn(async () => {});
    useChangesStore.setState({ refresh: changes, repoId: null, status: null });
    useBranchStore.setState({ refresh: branches });
    useHistoryStore.setState({ refresh: vi.fn(async () => {}) });
    const sync = vi.fn(async () => {});
    useSyncStore.setState({ refresh: sync });

    const { unmount } = render(<Probe id="r1" />);
    expect(changes).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(emit).not.toBeNull());

    emit?.("other-repo");
    expect(changes).toHaveBeenCalledTimes(1);
    emit?.("r1");
    expect(changes).toHaveBeenCalledTimes(2);
    expect(branches).toHaveBeenCalledTimes(2);
    expect(sync).toHaveBeenCalledTimes(2);

    window.dispatchEvent(new Event("focus"));
    expect(changes).toHaveBeenCalledTimes(3);

    unmount();
    expect(unlisten).toHaveBeenCalled();
    window.dispatchEvent(new Event("focus"));
    expect(changes).toHaveBeenCalledTimes(3);
  });
});
