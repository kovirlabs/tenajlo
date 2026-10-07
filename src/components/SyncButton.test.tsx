import { afterEach, describe as suite, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { SyncState } from "../bindings";
import { useSyncStore } from "../stores/syncStore";
import { describe, SyncButton } from "./SyncButton";

const state = (action: SyncState["action"], ahead = 0, behind = 0): SyncState => ({
  action,
  branch: "main",
  ahead,
  behind,
  lastFetched: null,
});

suite("sync button", () => {
  afterEach(cleanup);

  it("labels each action in plain language", () => {
    expect(describe(state({ type: "NoRemote" }))).toBeNull();
    expect(describe(state({ type: "Publish", remote: "origin" }))?.title).toBe("Publish branch");
    expect(describe(state({ type: "Pull", remote: "origin" }, 0, 3))).toMatchObject({
      title: "Pull origin",
      badge: "↓3",
      detail: "Never fetched",
    });
    expect(describe(state({ type: "Pull", remote: "origin" }, 2, 3))?.badge).toBe("↓3 ↑2");
    expect(describe(state({ type: "Push", remote: "origin" }, 2))?.badge).toBe("↑2");
    expect(describe(state({ type: "Fetch", remote: "origin" }))?.request).toBe("Fetch");
  });

  it("runs the action and shows progress with cancel", () => {
    const run = vi.fn(async () => {});
    const cancel = vi.fn();
    useSyncStore.setState({
      state: state({ type: "Push", remote: "origin" }, 1),
      running: null,
      run,
      cancel,
    });
    render(<SyncButton />);
    fireEvent.click(screen.getByRole("button", { name: /Push origin/ }));
    expect(run).toHaveBeenCalledWith("Push");

    act(() => {
      useSyncStore.setState({
        running: {
          opId: "op",
          request: "Push",
          progress: { phase: "Writing objects", percent: 40, detail: null, remote: false },
        },
      });
    });
    expect(screen.getByText("Writing objects 40%")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(cancel).toHaveBeenCalled();
  });
});
