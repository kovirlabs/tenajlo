import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { FileChange } from "../bindings";
import { useChangesStore } from "../stores/changesStore";
import { ChangesList } from "./ChangesList";

const discardChanges = vi.fn();
const ignoreFile = vi.fn();
const openRepoFile = vi.fn(async () => ({ status: "ok", data: null }));
const revealRepoFile = vi.fn(async () => ({ status: "ok", data: null }));
vi.mock("../api/merge", () => ({
  openRepoFile: (...a: unknown[]) => (openRepoFile as (...x: unknown[]) => unknown)(...a),
  revealRepoFile: (...a: unknown[]) => (revealRepoFile as (...x: unknown[]) => unknown)(...a),
}));
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
  // Like the real one: runs the operation and resolves to its result.
  const mutate = vi.fn((op: (id: string) => Promise<unknown>) => op("r1"));
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

  it("opens the file in the editor or shows it in its folder", async () => {
    setup([file("plc/main.st", "Modified")]);
    fireEvent.contextMenu(screen.getByText("plc/main.st"));
    fireEvent.click(screen.getByRole("menuitem", { name: "Open in editor" }));
    expect(openRepoFile).toHaveBeenCalledWith("r1", "plc/main.st");
    fireEvent.contextMenu(screen.getByText("plc/main.st"));
    fireEvent.click(screen.getByRole("menuitem", { name: "Show in folder" }));
    expect(revealRepoFile).toHaveBeenCalledWith("r1", "plc/main.st");
  });
});
