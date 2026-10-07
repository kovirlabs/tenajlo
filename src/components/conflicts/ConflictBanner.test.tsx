import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { OperationState } from "../../bindings";
import { useConflictStore } from "../../stores/conflictStore";
import { ConflictBanner } from "./ConflictBanner";

const api = vi.hoisted(() => ({ openRepoFile: vi.fn(async () => ({ status: "ok", data: null })) }));
vi.mock("../../api/merge", () => api);

const markResolved = vi.fn(async () => {});
const abort = vi.fn(async () => {});

function at(els: HTMLElement[], i: number): HTMLElement {
  const el = els.at(i);
  if (!el) throw new Error(`no element ${i}`);
  return el;
}

function show(state: OperationState) {
  useConflictStore.setState({ repoId: "r", state, markResolved, abort });
  render(<ConflictBanner repoId="r" />);
}

describe("ConflictBanner", () => {
  beforeEach(() => vi.clearAllMocks());
  afterEach(cleanup);

  it("lists conflicts, opens files and confirms when markers remain", () => {
    show({
      operation: "Merge",
      mergeSummary: "Merge branch 'main' of https://h/x",
      conflicts: [
        { path: "plc/main.st", markers: 2 },
        { path: "notes.md", markers: 0 },
      ],
    });
    expect(screen.getByText(/Resolve these 2 conflicts/)).toBeTruthy();
    expect(screen.getByText("2 conflicts")).toBeTruthy();

    fireEvent.click(at(screen.getAllByRole("button", { name: "Open" }), 0));
    expect(api.openRepoFile).toHaveBeenCalledWith("r", "plc/main.st");

    const marks = screen.getAllByRole("button", { name: "Mark resolved" });
    fireEvent.click(at(marks, 1));
    expect(markResolved).toHaveBeenCalledWith(["notes.md"]);

    fireEvent.click(at(marks, 0));
    expect(markResolved).toHaveBeenCalledTimes(1);
    expect(screen.getByText("This file still has conflict markers")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Mark resolved anyway" }));
    expect(markResolved).toHaveBeenCalledWith(["plc/main.st"]);
  });

  it("asks to commit once resolved, and confirms abort", () => {
    show({ operation: "Merge", mergeSummary: null, conflicts: [] });
    expect(screen.getByText(/Commit to finish the merge/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Abort merge" }));
    expect(abort).not.toHaveBeenCalled();
    fireEvent.click(at(screen.getAllByRole("button", { name: "Abort merge" }), -1));
    expect(abort).toHaveBeenCalled();
  });

  it("explains a rebase started elsewhere", () => {
    show({ operation: "Rebase", mergeSummary: null, conflicts: [] });
    expect(screen.getByText(/rebase started outside Tenajlo/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Abort rebase" })).toBeTruthy();
  });

  it("is hidden when nothing is in progress", () => {
    show({ operation: null, mergeSummary: null, conflicts: [] });
    expect(screen.queryByRole("region", { name: "Conflicts" })).toBeNull();
  });
});
