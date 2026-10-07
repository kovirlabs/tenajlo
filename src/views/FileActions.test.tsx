import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { FileChange } from "../bindings";
import { useChangesStore } from "../stores/changesStore";
import { ChangesList } from "./ChangesList";

const discardChanges = vi.fn();
const ignoreFile = vi.fn();
vi.mock("../api/changes", () => ({
  discardChanges: (...a: unknown[]) => discardChanges(...a),
  ignoreFile: (...a: unknown[]) => ignoreFile(...a),
  setStaged: vi.fn(),
}));

const file = (path: string, kind: FileChange["kind"]): FileChange => ({
  path,
  oldPath: null,
  kind,
  staged: "None",
  submodule: false,
});

function setup(files: FileChange[]) {
  const mutate = vi.fn(async (op: (id: string) => Promise<unknown>) => {
    await op("r1");
    return true;
  });
  useChangesStore.setState({
    repoId: "r1",
    busy: false,
    error: null,
    selectedPath: null,
    mutate: mutate as never,
    status: {
      branch: { name: "main", tip: "x", upstream: null, ahead: 0, behind: 0, upstreamGone: false },
      files,
      hasConflicts: false,
    },
  });
  render(<ChangesList />);
}

describe("file context menu", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("discards only after confirmation", () => {
    setup([file("a.txt", "Modified")]);
    fireEvent.contextMenu(screen.getByText("a.txt"));
    expect(screen.queryByText(/Ignore/)).toBeNull();
    fireEvent.click(screen.getByRole("menuitem", { name: "Discard changes…" }));
    expect(discardChanges).not.toHaveBeenCalled();
    expect(screen.getByText(/moved to the Trash/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Discard changes" }));
    expect(discardChanges).toHaveBeenCalledWith("r1", ["a.txt"]);
  });

  it("offers ignore options for new files", () => {
    setup([file("logs/run.log", "Untracked")]);
    fireEvent.contextMenu(screen.getByText("logs/run.log"));
    fireEvent.click(screen.getByRole("menuitem", { name: "Ignore all .log files" }));
    expect(ignoreFile).toHaveBeenCalledWith("r1", "logs/run.log", true);
  });

  it("blocks discarding conflicted files", () => {
    setup([file("c.txt", "Conflicted")]);
    fireEvent.contextMenu(screen.getByText("c.txt"));
    const item = screen.getByRole("menuitem", { name: "Discard changes…" }) as HTMLButtonElement;
    expect(item.disabled).toBe(true);
  });
});
