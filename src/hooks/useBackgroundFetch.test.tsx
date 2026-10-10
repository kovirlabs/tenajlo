import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render } from "@testing-library/react";
import type { Settings, SyncState } from "../bindings";
import { useSettingsStore } from "../stores/settingsStore";
import { useSyncStore } from "../stores/syncStore";
import { useBackgroundFetch } from "./useBackgroundFetch";

const backgroundFetch = vi.fn<(repoId: string) => Promise<unknown>>(async () => ({
  status: "ok",
  data: null,
}));
vi.mock("../api/sync", () => ({ backgroundFetch: (id: string) => backgroundFetch(id) }));

const settings = (minutes: number): Settings => ({
  theme: "System",
  defaultCloneFolder: null,
  pullStrategy: "FastForwardOnly",
  backgroundFetchMinutes: minutes,
  notifyNewCommits: true,
  gitPath: null,
  editor: { kind: "SystemDefault" },
  welcomeCompleted: true,
  checkForUpdates: false,
});

function Probe() {
  useBackgroundFetch("r1");
  return null;
}

describe("useBackgroundFetch", () => {
  const refresh = vi.fn(async () => {});
  beforeEach(() => {
    vi.useFakeTimers();
    useSyncStore.setState({
      repoId: "r1",
      running: null,
      refresh,
      state: { action: { type: "Fetch", remote: "origin" } } as unknown as SyncState,
    });
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it("fetches on the configured interval and refreshes the sync button", async () => {
    useSettingsStore.setState({ settings: settings(5) });
    render(<Probe />);
    await act(async () => vi.advanceTimersByTimeAsync(4 * 60_000));
    expect(backgroundFetch).not.toHaveBeenCalled();
    await act(async () => vi.advanceTimersByTimeAsync(60_000));
    expect(backgroundFetch).toHaveBeenCalledWith("r1");
    expect(refresh).toHaveBeenCalledWith("r1");
  });

  it("skips while a sync runs and when turned off", async () => {
    useSettingsStore.setState({ settings: settings(1) });
    useSyncStore.setState({ running: { opId: "x", request: "Push", progress: null } });
    render(<Probe />);
    await act(async () => vi.advanceTimersByTimeAsync(60_000));
    expect(backgroundFetch).not.toHaveBeenCalled();
    cleanup();
    useSyncStore.setState({ running: null });
    useSettingsStore.setState({ settings: settings(0) });
    render(<Probe />);
    await act(async () => vi.advanceTimersByTimeAsync(10 * 60_000));
    expect(backgroundFetch).not.toHaveBeenCalled();
  });
});
