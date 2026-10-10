import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { FileChange, Identity } from "../bindings";
import { useChangesStore } from "../stores/changesStore";
import { useConflictStore } from "../stores/conflictStore";
import { CommitBox } from "./CommitBox";

const identity = vi.fn<() => Promise<{ status: "ok"; data: Identity }>>();
const commitChanges = vi.fn(async () => ({ status: "ok" as const, data: "abc" }));
const undoCommit = vi.fn(async () => ({
  status: "ok" as const,
  data: { summary: "Fix pump", description: "details" },
}));
vi.mock("../api/changes", () => ({
  getIdentity: () => identity(),
  commitChanges: (...a: unknown[]) => (commitChanges as (...x: unknown[]) => unknown)(...a),
  setGlobalIdentity: vi.fn(),
  undoCommit: (...a: unknown[]) => (undoCommit as (...x: unknown[]) => unknown)(...a),
}));

const file = (staged: FileChange["staged"]): FileChange => ({
  path: "a.txt",
  oldPath: null,
  kind: "Modified",
  staged,
  submodule: false,
});

function setup(staged: FileChange["staged"], id: Identity = { name: "E", email: "e@x" }) {
  identity.mockResolvedValue({ status: "ok", data: id });
  // Like the real one: runs the operation and resolves to its result.
  const mutate = vi.fn((op: (id: string) => Promise<unknown>) => op("r1"));
  useChangesStore.setState({
    repoId: "r1",
    busy: false,
    mutate: mutate as never,
    status: {
      branch: { name: "main", tip: "x", upstream: null, ahead: 0, behind: 0, upstreamGone: false },
      files: [file(staged)],
      hasConflicts: false,
    },
  });
  render(<CommitBox repoId="r1" />);
  return mutate;
}

describe("CommitBox", () => {
  beforeEach(() => {
    commitChanges.mockClear();
    useConflictStore.setState({ repoId: "r1", state: null });
  });

  it("commits with Ctrl+Enter and clears the message", async () => {
    setup("Full");
    const summary = screen.getByLabelText("Commit summary") as HTMLInputElement;
    fireEvent.change(summary, { target: { value: "Fix pump" } });
    await waitFor(() => expect(identity).toHaveBeenCalled());
    fireEvent.keyDown(summary, { key: "Enter", ctrlKey: true });
    await waitFor(() => expect(commitChanges).toHaveBeenCalledWith("r1", "Fix pump", ""));
    await waitFor(() => expect(summary.value).toBe(""));
  });

  it("offers Undo while the new commit is the tip, restoring the message", async () => {
    setup("Full");
    const summary = screen.getByLabelText("Commit summary") as HTMLInputElement;
    fireEvent.change(summary, { target: { value: "Fix pump" } });
    await waitFor(() => expect(identity).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: /Commit to main/ }));
    await waitFor(() => expect(summary.value).toBe(""));
    // The store refresh would move the tip to the new commit; simulate it.
    useChangesStore.setState((s) => ({
      status: s.status && { ...s.status, branch: { ...s.status.branch, tip: "abc" } },
    }));
    fireEvent.click(await screen.findByRole("button", { name: "Undo" }));
    await waitFor(() => expect(undoCommit).toHaveBeenCalledWith("r1", "abc"));
    await waitFor(() => expect(summary.value).toBe("Fix pump"));
    expect((screen.getByLabelText("Commit description") as HTMLTextAreaElement).value).toBe(
      "details",
    );
  });

  it("is disabled with nothing staged or no summary", () => {
    setup("None");
    fireEvent.change(screen.getByLabelText("Commit summary"), { target: { value: "x" } });
    const button = screen.getByRole("button", { name: /Commit to main/ }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });

  it("asks for an identity instead of committing when none is set", async () => {
    setup("Full", { name: null, email: null });
    expect(await screen.findByText(/needs your name and email/)).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Commit summary"), { target: { value: "x" } });
    fireEvent.click(screen.getByRole("button", { name: /Commit to main/ }));
    expect(commitChanges).not.toHaveBeenCalled();
    expect(screen.getByText(/global Git settings/)).toBeTruthy();
  });

  it("finishes a merge with git's message even when nothing is staged", async () => {
    useConflictStore.setState({
      repoId: "r1",
      state: { operation: "Merge", conflicts: [], mergeSummary: "Merge branch 'main' of h" },
    });
    setup("None");
    const summary = screen.getByLabelText("Commit summary") as HTMLInputElement;
    expect(summary.value).toBe("Merge branch 'main' of h");
    await waitFor(() => expect(identity).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: /Commit merge to main/ }));
    await waitFor(() =>
      expect(commitChanges).toHaveBeenCalledWith("r1", "Merge branch 'main' of h", ""),
    );
  });

  it("blocks committing during a rebase started elsewhere", () => {
    useConflictStore.setState({
      repoId: "r1",
      state: { operation: "Rebase", conflicts: [], mergeSummary: null },
    });
    setup("Full");
    fireEvent.change(screen.getByLabelText("Commit summary"), { target: { value: "x" } });
    expect(
      (screen.getByRole("button", { name: /Commit to main/ }) as HTMLButtonElement).disabled,
    ).toBe(true);
  });
});
