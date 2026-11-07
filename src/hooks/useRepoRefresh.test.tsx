import { afterEach, describe, expect, it, vi } from "vitest";
import { act, render, waitFor } from "@testing-library/react";
import { useBranchStore } from "../stores/branchStore";
import { useChangesStore } from "../stores/changesStore";
import { useHistoryStore } from "../stores/historyStore";
import { useSyncStore } from "../stores/syncStore";
import { useConflictStore } from "../stores/conflictStore";
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
    const conflicts = vi.fn(async () => {});
    useConflictStore.setState({ refresh: conflicts });

    const { unmount } = render(<Probe id="r1" />);
    expect(changes).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(emit).not.toBeNull());

    emit?.("other-repo");
    expect(changes).toHaveBeenCalledTimes(1);
    emit?.("r1");
    expect(changes).toHaveBeenCalledTimes(2);
    expect(branches).toHaveBeenCalledTimes(2);
    expect(sync).toHaveBeenCalledTimes(2);
    expect(conflicts).toHaveBeenCalledTimes(2);

    window.dispatchEvent(new Event("focus"));
    expect(changes).toHaveBeenCalledTimes(3);

    unmount();
    expect(unlisten).toHaveBeenCalled();
    window.dispatchEvent(new Event("focus"));
    expect(changes).toHaveBeenCalledTimes(3);
  });

  it("reloads history when a push changes what's on the server, not just on new commits", async () => {
    const history = vi.fn(async () => {});
    useHistoryStore.setState({ refresh: history });
    useChangesStore.setState({ refresh: vi.fn(async () => {}) });
    useBranchStore.setState({ refresh: vi.fn(async () => {}) });
    useSyncStore.setState({ refresh: vi.fn(async () => {}) });
    useConflictStore.setState({ refresh: vi.fn(async () => {}) });
    const branch = (ahead: number) => ({
      name: "main",
      tip: "abc",
      upstream: "origin/main",
      ahead,
      behind: 0,
      upstreamGone: false,
    });
    const status = (ahead: number) => ({ branch: branch(ahead), files: [], hasConflicts: false });
    useChangesStore.setState({ repoId: "r1", status: status(1) });

    render(<Probe id="r1" />);
    await waitFor(() => expect(history).toHaveBeenCalledTimes(1));
    // Pushed: same HEAD, nothing ahead any more, so the "Not pushed" marker must go.
    act(() => useChangesStore.setState({ status: status(0) }));
    await waitFor(() => expect(history).toHaveBeenCalledTimes(2));
    // A refresh that changes nothing doesn't reload.
    act(() => useChangesStore.setState({ status: status(0) }));
    expect(history).toHaveBeenCalledTimes(2);
  });
});
