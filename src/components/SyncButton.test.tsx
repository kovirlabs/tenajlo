import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import type { SyncState } from "../bindings";
import { useSyncStore } from "../stores/syncStore";
import { useUiStore } from "../stores/uiStore";
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
      title: "Get 3 commits",
      badge: "↓3",
      detail: "Never checked",
      tooltip: "Your branch is 3 commits behind origin. Click to get them (git pull).",
    });
    const diverged = syncLabel(state({ type: "Pull", remote: "origin" }, 2, 3));
    expect(diverged?.badge).toBe("↓3 ↑2");
    expect(diverged?.tooltip).toMatch(/get theirs first/);
    expect(syncLabel(state({ type: "Push", remote: "origin" }, 1))).toMatchObject({
      title: "Send 1 commit",
      badge: "↑1",
      tooltip: "You have 1 commit that isn't on origin yet. Click to send it (git push).",
    });
    expect(syncLabel(state({ type: "Fetch", remote: "origin" }))).toMatchObject({
      request: "Fetch",
      title: "Check for changes",
    });
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
    fireEvent.click(screen.getByRole("button", { name: /Send 1 commit/ }));
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

  it("offers to publish a repository that has no remote", () => {
    const run = vi.fn(async () => {});
    useSyncStore.setState({ state: state({ type: "NoRemote" }), running: null, run });
    useUiStore.setState({ dialog: null });
    render(<SyncButton />);
    fireEvent.click(screen.getByRole("button", { name: /Publish repository/ }));
    expect(useUiStore.getState().dialog).toBe("publishRepository");
    expect(run).not.toHaveBeenCalled();
  });
});
