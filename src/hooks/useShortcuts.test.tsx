import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render } from "@testing-library/react";
import type { SyncState } from "../bindings";
import { useSyncStore } from "../stores/syncStore";
import { useUiStore } from "../stores/uiStore";
import { useShortcuts } from "./useShortcuts";

function Probe() {
  useShortcuts(true);
  return null;
}

const press = (key: string) => fireEvent.keyDown(window, { key, ctrlKey: true, shiftKey: true });

describe("useShortcuts", () => {
  const run = vi.fn(async () => {});
  beforeEach(() => {
    run.mockClear();
    useUiStore.setState({ dialog: null });
    useSyncStore.setState({
      run,
      state: { action: { type: "Push", remote: "origin" } } as unknown as SyncState,
    });
  });
  afterEach(cleanup);

  it("pushes and opens New Branch when no dialog is open", () => {
    render(<Probe />);
    press("P");
    expect(run).toHaveBeenCalledWith("Push");
    press("N");
    expect(useUiStore.getState().dialog).toBe("newBranch");
  });

  it("ignores push and new-branch shortcuts while a dialog is open", () => {
    useUiStore.setState({ dialog: "settings" });
    render(<Probe />);
    press("P");
    press("N");
    expect(run).not.toHaveBeenCalled();
    expect(useUiStore.getState().dialog).toBe("settings");
  });

  it("ignores them under a confirmation or error, which the UI store doesn't track", () => {
    render(<Probe />);
    const dialog = document.createElement("dialog");
    dialog.setAttribute("open", "");
    document.body.append(dialog);
    press("P");
    press("N");
    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    expect(run).not.toHaveBeenCalled();
    expect(useUiStore.getState().dialog).toBeNull();
    dialog.remove();
  });
});
