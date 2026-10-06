import { afterEach, describe, expect, it } from "vitest";
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

  it("shows an empty state", () => {
    setStatus([]);
    render(<ChangesList />);
    expect(screen.getByText("No changes")).toBeTruthy();
  });
});
