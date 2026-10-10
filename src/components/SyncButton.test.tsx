import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import type { SyncState } from "../bindings";
import { useSyncStore } from "../stores/syncStore";
import { SyncButton, syncLabel } from "./SyncButton";

const state = (action: SyncState["action"], ahead = 0, behind = 0): SyncState => ({
  action,
  branch: "main",
  ahead,
  behind,
  lastFetched: null,
});

describe("sync button", () => {
  it("labels each action in plain language", () => {
    expect(syncLabel(state({ type: "NoRemote" }))).toBeNull();
    expect(syncLabel(state({ type: "Publish", remote: "origin" }))?.title).toBe("Publish branch");
    expect(syncLabel(state({ type: "Pull", remote: "origin" }, 0, 3))).toMatchObject({
      title: "Pull origin",
      badge: "↓3",
      detail: "Never fetched",
    });
    expect(syncLabel(state({ type: "Pull", remote: "origin" }, 2, 3))?.badge).toBe("↓3 ↑2");
    expect(syncLabel(state({ type: "Push", remote: "origin" }, 2))?.badge).toBe("↑2");
    expect(syncLabel(state({ type: "Fetch", remote: "origin" }))?.request).toBe("Fetch");
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
