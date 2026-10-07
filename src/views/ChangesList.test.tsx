import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { FileChange } from "../bindings";
import { useChangesStore } from "../stores/changesStore";
import { ChangesList } from "./ChangesList";

const file = (path: string, over: Partial<FileChange> = {}): FileChange => ({
  path,
  oldPath: null,
  kind: "Modified",
  staged: "None",
  submodule: false,
  ...over,
});

const setStatus = (files: FileChange[]) =>
  useChangesStore.setState({
    repoId: "r",
    error: null,
    selectedPath: files[0]?.path ?? null,
    status: {
      branch: {
        name: "main",
        tip: "abc",
        upstream: null,
        ahead: 0,
        behind: 0,
        upstreamGone: false,
      },
      files,
      hasConflicts: false,
    },
  });

describe("ChangesList", () => {
  afterEach(cleanup);

  it("lists files with status labels and selects on click", () => {
    setStatus([
      file("a.txt"),
      file("new.txt", { kind: "Untracked" }),
      file("b.txt", { staged: "Partial" }),
    ]);
    render(<ChangesList />);
    expect(screen.getByText("3 changed files")).toBeTruthy();
    expect(screen.getByLabelText("New file")).toBeTruthy();
    const partial = screen.getByLabelText("Include b.txt") as HTMLInputElement;
    expect(partial.indeterminate).toBe(true);

    fireEvent.click(screen.getByText("new.txt"));
    expect(useChangesStore.getState().selectedPath).toBe("new.txt");
  });

  it("stages a file and toggles all files", () => {
    setStatus([
      file("a.txt"),
      file("b.txt", { staged: "Full" }),
      file("c.txt", { kind: "Conflicted" }),
    ]);
    const setStaged = vi.fn(async () => {});
    useChangesStore.setState({ setStaged, busy: false });
    render(<ChangesList />);

    fireEvent.click(screen.getByLabelText("Include a.txt"));
    expect(setStaged).toHaveBeenLastCalledWith(["a.txt"], true);
    fireEvent.click(screen.getByLabelText("Include b.txt"));
    expect(setStaged).toHaveBeenLastCalledWith(["b.txt"], false);
    expect((screen.getByLabelText("Include c.txt") as HTMLInputElement).disabled).toBe(true);

    // Partially staged overall → clicking stages everything except the conflict.
    fireEvent.click(screen.getByLabelText("Include all files"));
    expect(setStaged).toHaveBeenLastCalledWith(["a.txt", "b.txt"], true);
    // Clicking a checkbox doesn't change which file is selected.
    expect(useChangesStore.getState().selectedPath).toBe("a.txt");
  });

  it("shows an empty state", () => {
    setStatus([]);
    render(<ChangesList />);
    expect(screen.getByText("No changes")).toBeTruthy();
  });
});
